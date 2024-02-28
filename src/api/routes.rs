mod statement;
mod transaction;

use axum::http::StatusCode;
pub use statement::show as show_balance;
pub use statement::Response as StatementResponse;
pub use transaction::create as create_transaction;
pub use transaction::Request as TransactionRequest;
pub use transaction::Response as TransactionResponse;

use crate::persistence;
use crate::telemetry;

impl From<persistence::Error> for StatusCode {
    fn from(err: persistence::Error) -> Self {
        telemetry::error!("Database error: {:?}", err);

        match err {
            persistence::Error::ClientNotFound => StatusCode::NOT_FOUND,
            persistence::Error::BalanceConstraintViolation => StatusCode::UNPROCESSABLE_ENTITY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(persistence::Error::ClientNotFound, StatusCode::NOT_FOUND)]
    #[case(persistence::Error::Connection, StatusCode::INTERNAL_SERVER_ERROR)]
    #[case(persistence::Error::Internal("internal".into()), StatusCode::INTERNAL_SERVER_ERROR)]
    #[case(
        persistence::Error::BalanceConstraintViolation,
        StatusCode::UNPROCESSABLE_ENTITY
    )]
    fn test_from_persistence_error(
        #[case] error: persistence::Error,
        #[case] expected_status: StatusCode,
    ) {
        assert_eq!(expected_status, StatusCode::from(error));
    }
}
