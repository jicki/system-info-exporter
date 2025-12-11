use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;
use tracing::info;

use crate::config::Settings;
use crate::metrics;

mod handlers;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

pub async fn serve(settings: Settings) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/healthz", get(health))
        .route("/ready", get(health))
        .route("/metrics", get(handlers::get_metrics));

    let addr = SocketAddr::new(
        settings.server.host.parse()?,
        settings.server.port,
    );

    info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
