use crate::api::routes::{StatementResponse, TransactionRequest, TransactionResponse};
use axum::async_trait;

#[derive(Debug)]
pub enum Error {
    Connection,
    Internal(String),
    ClientNotFound,
    BalanceConstraintViolation,
}

#[async_trait]
pub trait Repository: Send + Sync {
    async fn create_transaction(
        &self,
        client_id: &i16,
        data: &TransactionRequest,
    ) -> Result<TransactionResponse, Error>;

    async fn get_balance(&self, client_id: &i16) -> Result<StatementResponse, Error>;
}
