use axum::Json;

use crate::metrics::{self, SystemMetrics};

pub async fn get_metrics() -> Json<SystemMetrics> {
    Json(metrics::collect())
}
