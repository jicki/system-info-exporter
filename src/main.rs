use anyhow::Result;
use tracing::info;

mod api;
mod config;
mod error;
mod metrics;

/// NVML library search paths
const NVML_LIB_PATHS: &[&str] = &[
    "/host/nvidia-libs",
    "/usr/lib/x86_64-linux-gnu",
    "/usr/lib",
];

/// Setup LD_LIBRARY_PATH for NVML library loading
/// This must be called before any dlopen() calls to take effect
fn setup_nvml_library_path() {
    for dir in NVML_LIB_PATHS {
        let lib_path = format!("{}/libnvidia-ml.so", dir);
        let lib_path_v1 = format!("{}/libnvidia-ml.so.1", dir);
        if std::path::Path::new(&lib_path).exists()
            || std::path::Path::new(&lib_path_v1).exists()
        {
            // Prepend to existing LD_LIBRARY_PATH
            let current = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
            let new_path = if current.is_empty() {
                dir.to_string()
            } else {
                format!("{}:{}", dir, current)
            };
            std::env::set_var("LD_LIBRARY_PATH", &new_path);
            eprintln!("Set LD_LIBRARY_PATH={}", new_path);
            return;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup NVML library path FIRST, before any dynamic library loading
    setup_nvml_library_path();

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
