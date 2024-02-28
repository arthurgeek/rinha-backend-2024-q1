use std::{collections::BTreeMap, ops::Deref};

use axum::async_trait;
use bb8_postgres::{
    bb8::{self, CustomizeConnection},
    tokio_postgres, PostgresConnectionManager,
};

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub enum Statement {
    CreateDebitTransaction,
    CreateCreditTransaction,
    GetBalance,
}

#[derive(Debug)]
pub struct Cache;

#[async_trait]
impl CustomizeConnection<Connection, tokio_postgres::Error> for Cache {
    async fn on_acquire(&self, conn: &mut Connection) -> Result<(), tokio_postgres::Error> {
        conn.statements.insert(
            Statement::CreateDebitTransaction,
            conn.prepare("SELECT * FROM debitar($1, $2, $3);").await?,
        );

        conn.statements.insert(
            Statement::CreateCreditTransaction,
            conn.prepare("SELECT * FROM creditar($1, $2, $3);").await?,
        );

        conn.statements.insert(
            Statement::GetBalance,
            conn.prepare(
                r#"
                    SELECT
                        c.saldo,
                        c.limite,
                        t.valor,
                        t.tipo,
                        t.descricao,
                        t.realizada_em
                    FROM
                        clientes c
                    LEFT JOIN (
                        SELECT
                            cliente_id,
                            valor,
                            tipo,
                            descricao,
                            realizada_em
                        FROM
                            transacoes
                        WHERE
                            cliente_id = $1
                        ORDER BY
                            id DESC
                        LIMIT 10
                    ) AS t ON c.id = t.cliente_id
                    WHERE
                        c.id = $1;
                "#,
            )
            .await?,
        );

        Ok(())
    }
}

pub struct Connection {
    inner: tokio_postgres::Client,
    pub statements: BTreeMap<Statement, tokio_postgres::Statement>,
}

impl Connection {
    fn new(inner: tokio_postgres::Client) -> Self {
        Self {
            inner,
            statements: Default::default(),
        }
    }
}

impl Deref for Connection {
    type Target = tokio_postgres::Client;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ConnectionManager<Tls>
where
    Tls: tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket>,
{
    inner: PostgresConnectionManager<Tls>,
}

impl<Tls> ConnectionManager<Tls>
where
    Tls: tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket>,
{
    pub fn new(config: tokio_postgres::Config, tls: Tls) -> Self {
        Self {
            inner: PostgresConnectionManager::new(config, tls),
        }
    }
}

#[async_trait]
impl<Tls> bb8::ManageConnection for ConnectionManager<Tls>
where
    Tls: tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket>  + Clone + Send + Sync + 'static,
    <Tls as tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket>>::Stream: Send + Sync,
    <Tls as tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket>>::TlsConnect: Send,
    <<Tls as tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket>>::TlsConnect as tokio_postgres::tls::TlsConnect<tokio_postgres::Socket>>::Future: Send,
{
    type Connection = Connection;
    type Error = tokio_postgres::Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let conn = self.inner.connect().await?;
        Ok(Connection::new(conn))
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.simple_query("").await.map(|_| ())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.inner.has_broken(&mut conn.inner)
    }
}
