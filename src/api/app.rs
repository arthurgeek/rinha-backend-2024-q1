use super::routes;
use crate::persistence::Repository;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn new(repo: Arc<dyn Repository>) -> Router {
    Router::new()
        .route("/clientes/:id/transacoes", post(routes::create_transaction))
        .route("/clientes/:id/extrato", get(routes::show_balance))
        .with_state(repo)
}
