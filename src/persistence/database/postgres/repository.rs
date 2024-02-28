use super::statements_cache;
use crate::{
    api::routes::{StatementResponse, TransactionRequest, TransactionResponse},
    models::{Balance, Transaction},
    persistence::{Error, Repository as RepositoryTrait},
    telemetry,
};
use axum::async_trait;
use bb8_postgres::{
    bb8::{self, Pool, PooledConnection},
    tokio_postgres::{self},
};
use std::str::FromStr;

#[derive(Clone)]
pub struct Repository {
    pool: Pool<statements_cache::ConnectionManager<tokio_postgres::NoTls>>,
}

impl Repository {
    pub async fn new(host: &str) -> Result<Self, Error> {
        let manager = statements_cache::ConnectionManager::new(
            tokio_postgres::Config::from_str(&format!(
                "host={} user=admin password=123 dbname=rinha",
                host
            ))?,
            tokio_postgres::NoTls,
        );

        let pool = Pool::builder()
            .max_size(40)
            .min_idle(Some(40))
            .connection_customizer(Box::new(statements_cache::Cache))
            .connection_timeout(std::time::Duration::from_secs(5))
            .build(manager)
            .await?;

        Ok(Self { pool })
    }

    pub async fn connection(
        &self,
    ) -> Result<
        PooledConnection<'_, statements_cache::ConnectionManager<tokio_postgres::NoTls>>,
        Error,
    > {
        let conn = self.pool.get().await?;
        Ok(conn)
    }
}

#[async_trait]
impl RepositoryTrait for Repository {
    async fn create_transaction(
        &self,
        client_id: &i16,
        data: &TransactionRequest,
    ) -> Result<TransactionResponse, Error> {
        let conn = self.connection().await?;

        let stmt = match data.tipo.as_str() {
            "c" => statements_cache::Statement::CreateCreditTransaction,
            "d" => statements_cache::Statement::CreateDebitTransaction,
            _ => return Err(Error::Internal("Invalid transaction type".into())),
        };

        let row = conn
            .query_one(
                conn.statements
                    .get(&stmt)
                    .ok_or(Error::Internal("Statement not found".into()))?,
                &[&client_id, &data.valor, &data.descricao],
            )
            .await?;

        row.try_into()
    }

    async fn get_balance(&self, client_id: &i16) -> Result<StatementResponse, Error> {
        let conn = self.connection().await?;

        let rows = conn
            .query(
                conn.statements
                    .get(&statements_cache::Statement::GetBalance)
                    .ok_or(Error::Internal("Statement not found".into()))?,
                &[&client_id],
            )
            .await?;

        rows.try_into()
    }
}

impl TryFrom<tokio_postgres::Row> for TransactionResponse {
    type Error = Error;

    fn try_from(row: tokio_postgres::Row) -> Result<Self, Self::Error> {
        let result: i16 = row.try_get("resultado_codigo")?;

        match result {
            0 => {
                let balance: i32 = row.try_get("resultado_saldo")?;
                let limit: i32 = row.try_get("resultado_limite")?;

                Ok(Self {
                    saldo: balance,
                    limite: limit,
                })
            }
            1 => Err(Error::ClientNotFound),
            2 => Err(Error::BalanceConstraintViolation),
            _ => Err(Error::Internal("Unknown result code".into())),
        }
    }
}

impl TryFrom<Vec<tokio_postgres::Row>> for StatementResponse {
    type Error = Error;

    fn try_from(rows: Vec<tokio_postgres::Row>) -> Result<Self, Self::Error> {
        let balance: i32 = rows
            .first()
            .ok_or(Error::ClientNotFound)?
            .try_get("saldo")?;

        let limit: i32 = rows.first().unwrap().try_get("limite")?;

        let amount = rows.first().unwrap().try_get::<_, i16>("valor");

        let mut transactions = Vec::with_capacity(rows.len());

        if amount.is_ok() {
            for row in rows {
                transactions.push(Transaction {
                    valor: row.try_get("valor")?,
                    tipo: row.try_get("tipo")?,
                    descricao: row.try_get("descricao")?,
                    realizada_em: row.try_get("realizada_em")?,
                });
            }
        }

        Ok(Self {
            saldo: Balance {
                total: balance,
                data_extrato: std::time::SystemTime::now(),
                limite: limit,
            },
            ultimas_transacoes: transactions,
        })
    }
}

impl From<bb8::RunError<tokio_postgres::Error>> for Error {
    fn from(err: bb8::RunError<tokio_postgres::Error>) -> Self {
        telemetry::error!("Postgres error: {:?}", err);

        match err {
            bb8::RunError::User(e) => Self::Internal(e.to_string()),
            bb8::RunError::TimedOut => Self::Connection,
        }
    }
}

impl From<tokio_postgres::Error> for Error {
    fn from(err: tokio_postgres::Error) -> Self {
        telemetry::error!("Postgres error: {:?}", err);

        Self::Internal(err.to_string())
    }
}
