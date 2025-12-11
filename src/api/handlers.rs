use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::metrics::{self, NodeMetrics, SystemMetrics};

pub async fn get_metrics() -> Json<SystemMetrics> {
    Json(metrics::collect())
}

pub async fn get_node_metrics() -> Json<NodeMetrics> {
    Json(NodeMetrics::collect())
}

pub async fn get_prometheus_metrics() -> Response {
    let metrics = NodeMetrics::collect();
    let body = metrics.to_prometheus();

    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
        .into_response()
}
