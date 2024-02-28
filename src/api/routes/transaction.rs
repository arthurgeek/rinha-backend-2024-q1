use std::sync::Arc;

use crate::{persistence::Repository, telemetry};
use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, Path, Request as AxumRequest, State},
    http::StatusCode,
    Json,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Deserialize)]
pub struct Request {
    pub valor: i16,
    pub tipo: String,
    pub descricao: String,
}

#[derive(Serialize)]
pub struct Response {
    pub limite: i32,
    pub saldo: i32,
}

pub async fn create(
    State(repo): State<Arc<dyn Repository>>,
    Path(id): Path<i16>,
    ValidateCreate(payload): ValidateCreate<Request>,
) -> Result<Json<Response>, StatusCode> {
    Ok(Json(repo.create_transaction(&id, &payload).await?))
}

pub struct ValidateCreate<Request>(pub Request);

#[async_trait]
impl<S> FromRequest<S> for ValidateCreate<Request>
where
    Request: DeserializeOwned,
    S: Send + Sync,
    Json<Request>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = StatusCode;

    async fn from_request(req: AxumRequest, state: &S) -> Result<Self, Self::Rejection> {
        let Json(data) = Json::from_request(req, state).await.map_err(
            #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
            |e| {
                telemetry::error!("Failed to deserialize request JSON: {}", e);

                StatusCode::UNPROCESSABLE_ENTITY
            },
        )?;

        match (data.descricao.len(), data.tipo.as_str()) {
            (1..=10, "c" | "d") => Ok(Self(data)),
            _ => {
                telemetry::error!("Invalid transaction kind or description");

                Err(StatusCode::UNPROCESSABLE_ENTITY)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{api::routes::StatementResponse, persistence::Error};
    use axum::{async_trait, body::Body};
    use http_body_util::BodyExt;
    use rstest::rstest;
    use serde_json::json;
    use tower::util::ServiceExt;

    #[rstest]
    #[case::valid(10, "c", "description")]
    fn test_deserialization(#[case] valor: i16, #[case] tipo: &str, #[case] descricao: &str) {
        let request = json!({
            "valor": valor,
            "tipo": tipo,
            "descricao": descricao,
        });

        let serialized_request = Request {
            valor,
            tipo: tipo.into(),
            descricao: descricao.into(),
        };

        assert_eq!(
            serde_json::from_value::<Request>(request).unwrap(),
            serialized_request
        );
    }

    #[rstest]
    #[case::valid(10, 100)]
    fn test_serialization(#[case] limite: i32, #[case] saldo: i32) {
        let expected_json = json!({
            "saldo": saldo,
            "limite": limite
        });

        assert_eq!(
            serde_json::to_value(Response { saldo, limite }).unwrap(),
            expected_json
        );
    }

    struct MockRepository {
        scenario: TestScenario,
    }

    #[derive(Clone)]
    enum TestScenario {
        ClientNotFound,
        InternalError,
        ConnectionError,
        Success(i32, i32),
    }

    #[async_trait]
    impl Repository for MockRepository {
        async fn get_balance(&self, _client_id: &i16) -> Result<StatementResponse, Error> {
            unimplemented!()
        }

        async fn create_transaction(
            &self,
            _client_id: &i16,
            _data: &Request,
        ) -> Result<Response, Error> {
            match self.scenario {
                TestScenario::ClientNotFound => Err(Error::ClientNotFound),
                TestScenario::InternalError => Err(Error::Internal("internal error".to_string())),
                TestScenario::ConnectionError => Err(Error::Connection),
                TestScenario::Success(limite, saldo) => Ok(Response { limite, saldo }),
            }
        }
    }

    #[rstest]
    #[case::invalid_tipo(
        json!({ "valor": 10, "tipo": "x", "descricao": "descricao" }),
        StatusCode::UNPROCESSABLE_ENTITY,
    )]
    #[case::invalid_valor(
        json!({ "valor": 1.2, "tipo": "c", "descricao": "descricao" }),
        StatusCode::UNPROCESSABLE_ENTITY,
    )]
    #[case::long_descricao(
        json!({ "valor": 10, "tipo": "c", "descricao": "descricao longa" }),
        StatusCode::UNPROCESSABLE_ENTITY,
    )]
    #[case::blank_descricao(
        json!({ "valor": 10, "tipo": "c", "descricao": "" }),
        StatusCode::UNPROCESSABLE_ENTITY,
    )]
    #[case::null_descricao(
        json!({ "valor": 10, "tipo": "c", "descricao": null }),
        StatusCode::UNPROCESSABLE_ENTITY,
    )]
    #[case::valid(
        json!({ "valor": 10, "tipo": "c", "descricao": "descricao" }),
        StatusCode::OK,
    )]
    #[tokio::test]
    async fn test_validation(
        #[case] request_json: serde_json::Value,
        #[case] expected_status: StatusCode,
    ) {
        let app = crate::api::app::new(Arc::new(MockRepository {
            scenario: TestScenario::Success(10, 100),
        }));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method(axum::http::Method::POST)
                    .uri("/clientes/1/transacoes")
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(request_json.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), expected_status);
    }

    #[rstest]
    #[case::client_not_found(TestScenario::ClientNotFound, StatusCode::NOT_FOUND)]
    #[case::internal_error(TestScenario::InternalError, StatusCode::INTERNAL_SERVER_ERROR)]
    #[case::connection_error(TestScenario::ConnectionError, StatusCode::INTERNAL_SERVER_ERROR)]
    #[case::success(TestScenario::Success(10, 100), StatusCode::OK)]
    #[tokio::test]
    async fn test_create(#[case] scenario: TestScenario, #[case] expected_status: StatusCode) {
        let app = crate::api::app::new(Arc::new(MockRepository {
            scenario: scenario.clone(),
        }));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method(axum::http::Method::POST)
                    .uri("/clientes/1/transacoes")
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({ "valor": 10, "tipo": "c", "descricao": "descricao" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), expected_status);

        let body = response.into_body().collect().await.unwrap().to_bytes();

        if let TestScenario::Success(limite, saldo) = scenario {
            let expected_json = json!({
                "saldo": saldo,
                "limite": limite,
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
