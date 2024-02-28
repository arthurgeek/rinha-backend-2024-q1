use std::sync::Arc;

use crate::{models, persistence::Repository};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct Response {
    pub saldo: models::Balance,
    pub ultimas_transacoes: Vec<models::Transaction>,
}

pub async fn show(
    State(repo): State<Arc<dyn Repository>>,
    Path(id): Path<i16>,
) -> Result<Json<Response>, StatusCode> {
    Ok(Json(repo.get_balance(&id).await?))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        api::{
            self,
            routes::{TransactionRequest, TransactionResponse},
        },
        persistence::Error,
    };
    use axum::{async_trait, body::Body, http::Request};
    use http_body_util::BodyExt;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use std::time::SystemTime;
    use tower::util::ServiceExt;

    #[fixture]
    fn transactions() -> Vec<models::Transaction> {
        vec![
            models::Transaction {
                valor: 1000,
                tipo: "c".into(),
                descricao: "salario".to_string(),
                realizada_em: SystemTime::UNIX_EPOCH,
            },
            models::Transaction {
                valor: 50,
                tipo: "d".into(),
                descricao: "bar".to_string(),
                realizada_em: SystemTime::UNIX_EPOCH,
            },
        ]
    }

    #[fixture]
    #[once]
    fn balance() -> models::Balance {
        models::Balance {
            total: 20,
            limite: 1000,
            data_extrato: SystemTime::UNIX_EPOCH,
        }
    }

    #[fixture]
    fn json(
        transactions: Vec<models::Transaction>,
        balance: &models::Balance,
    ) -> serde_json::Value {
        json!({
            "saldo": {
                "total": balance.total,
                "limite": balance.limite,
                "data_extrato": balance.data_extrato,
            },
            "ultimas_transacoes": transactions,
        })
    }

    #[rstest]
    #[case::without_transactions(
        vec![],
    )]
    #[case::with_transactions(transactions())]
    fn test_serialization(
        #[case] transactions: Vec<models::Transaction>,
        balance: &models::Balance,
    ) {
        let expected_json = json!({
            "saldo": {
                "total": balance.total,
                "limite": balance.limite,
                "data_extrato": balance.data_extrato,
            },
            "ultimas_transacoes": transactions
        });

        let response = Response {
            saldo: balance.to_owned(),
            ultimas_transacoes: transactions,
        };

        assert_eq!(serde_json::to_value(response).unwrap(), expected_json);
    }

    #[derive(Clone)]
    enum TestScenario {
        ClientNotFound,
        InternalError,
        ConnectionError,
        BalanceConstraintViolationError,
        Success(fn() -> Vec<models::Transaction>),
    }

    struct MockRepository {
        scenario: TestScenario,
    }

    #[async_trait]
    impl Repository for MockRepository {
        async fn get_balance(&self, _client_id: &i16) -> Result<Response, Error> {
            match self.scenario {
                TestScenario::ClientNotFound => Err(Error::ClientNotFound),
                TestScenario::InternalError => Err(Error::Internal("internal error".to_string())),
                TestScenario::ConnectionError => Err(Error::Connection),
                TestScenario::BalanceConstraintViolationError => {
                    Err(Error::BalanceConstraintViolation)
                }
                TestScenario::Success(transaction_generator) => Ok(Response {
                    saldo: balance(),
                    ultimas_transacoes: transaction_generator(),
                }),
            }
        }

        async fn create_transaction(
            &self,
            _client_id: &i16,
            _data: &TransactionRequest,
        ) -> Result<TransactionResponse, Error> {
            unimplemented!()
        }
    }

    #[rstest]
    #[case::client_not_found(TestScenario::ClientNotFound, StatusCode::NOT_FOUND)]
    #[case::internal_error(TestScenario::InternalError, StatusCode::INTERNAL_SERVER_ERROR)]
    #[case::connection_error(TestScenario::ConnectionError, StatusCode::INTERNAL_SERVER_ERROR)]
    #[case::balance_constraint_violation_error(
        TestScenario::BalanceConstraintViolationError,
        StatusCode::UNPROCESSABLE_ENTITY
    )]
    #[case::success_without_transactions(TestScenario::Success(Vec::new), StatusCode::OK)]
    #[case::success_with_transactions(TestScenario::Success(transactions as fn() -> Vec<models::Transaction>), StatusCode::OK)]
    #[tokio::test]
    async fn test_show(
        #[case] scenario: TestScenario,
        #[case] expected_status: StatusCode,
        balance: &models::Balance,
    ) {
        let app = api::app::new(Arc::new(MockRepository {
            scenario: scenario.clone(),
        }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/clientes/1/extrato")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), expected_status);

        let body = response.into_body().collect().await.unwrap().to_bytes();

        if let TestScenario::Success(transaction_generator) = scenario {
            let expected_json = json!({
                "saldo": {
                    "total": balance.total,
                    "limite": balance.limite,
                    "data_extrato": balance.data_extrato,
                },
                "ultimas_transacoes": transaction_generator(),
            });

            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                expected_json
            );
        } else {
            assert!(body.is_empty());
        }
    }
}
