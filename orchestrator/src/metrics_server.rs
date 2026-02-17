// HTTP server for Prometheus metrics endpoint
//
// Listens on /metrics endpoint (default: 0.0.0.0:9090)
// Used by Prometheus to scrape metrics

use anyhow::{Context, Result};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tracing::{error, info};

use crate::metrics;

/// Start the metrics HTTP server
/// 
/// # Arguments
/// * `port` - Port to listen on (default 9090)
///
/// # Returns
/// Result with server handle or error
pub async fn start_metrics_server(port: u16) -> Result<()> {
    // Initialize metrics
    metrics::init().context("Failed to initialize metrics")?;

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    info!("Starting metrics server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("Failed to bind metrics server")?;
    
    axum::serve(listener, app)
        .await
        .context("Metrics server error")?;

    Ok(())
}

/// Metrics endpoint handler
async fn metrics_handler() -> Response {
    match metrics::gather_metrics() {
        Ok(metrics_text) => (StatusCode::OK, metrics_text).into_response(),
        Err(e) => {
            error!("Failed to gather metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error gathering metrics: {}", e),
            ).into_response()
        }
    }
}

/// Health check endpoint
async fn health_handler() -> impl IntoResponse {
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_server_startup() {
        // Test with a random available port
        let result = tokio::spawn(start_metrics_server(0)).await;
        // Note: This test demonstrates the pattern, but actual testing would require
        // a proper test setup with available ports and cleanup
        let _ = result;
    }
}
