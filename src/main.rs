use std::sync::Arc;

mod api;
mod models;
mod persistence;
mod telemetry;

use hyper::body::Incoming;
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server,
};
use tower::Service;

#[cfg(feature = "telemetry")]
use {
    axum::{body::Body, http},
    tower_http::trace::TraceLayer,
    tower_request_id::{RequestId, RequestIdLayer},
    tracing::error_span,
    tracing_subscriber::{
        layer::{Layer, SubscriberExt},
        util::SubscriberInitExt,
    },
};

#[tokio::main]
async fn main() {
    #[cfg(feature = "telemetry")]
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
                .pretty()
                .with_filter(tracing_subscriber::filter::LevelFilter::DEBUG),
        )
        .init();

    let host = option_env!("DB_HOST").unwrap_or("localhost");

    let repo = persistence::database::Repository::new(host)
        .await
        .unwrap_or_else(|_| panic!("failed to connect to postgres database on: {}", host));

    let port = std::env::var("PORT").unwrap_or("3000".to_string());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap_or_else(|_| panic!("failed to bind listener to port: {}", port));

    telemetry::debug!(
        "Listening on {}",
        listener.local_addr().expect("failed to get local addr")
    );

    let app = api::app::new(Arc::new(repo));

    #[cfg(feature = "telemetry")]
    let app = app
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &http::Request<Body>| {
                let request_id = request
                    .extensions()
                    .get::<RequestId>()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown".into());

                error_span!(
                    "request",
                    id = %request_id,
                    method = %request.method(),
                    uri = %request.uri(),
                )
            }),
        )
        .layer(RequestIdLayer);

    // Continuously accept new connections.
    loop {
        let (socket, _remote_addr) = listener.accept().await.unwrap();
        let tower_service = app.clone();

        tokio::spawn(async move {
            let socket = TokioIo::new(socket);

            let hyper_service =
                hyper::service::service_fn(move |request: axum::extract::Request<Incoming>| {
                    tower_service.clone().call(request)
                });

            #[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
            if let Err(err) = server::conn::auto::Builder::new(TokioExecutor::new())
                .http2()
                .keep_alive_timeout(std::time::Duration::from_secs(120))
                .keep_alive_interval(std::time::Duration::from_secs(30))
                .timer(TokioTimer::new())
                .serve_connection(socket, hyper_service)
                .await
            {
                telemetry::error!("failed to serve connection: {}", err);
            }
        });
    }
}
