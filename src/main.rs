use anyhow::Result;
use tracing::info;

mod api;
mod config;
mod error;
mod metrics;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("system_info_exporter=info".parse().unwrap()),
        )
        .json()
        .init();

    info!("Starting system-info-exporter v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let settings = config::Settings::load()?;

    // Start the API server
    api::serve(settings).await?;

    Ok(())
}
