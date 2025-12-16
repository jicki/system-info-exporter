use config::{Config, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerSettings,
    pub metrics: MetricsSettings,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MetricsSettings {
    pub collect_interval_secs: u64,
    #[serde(default)]
    pub enabled: MetricsEnabled,
}

/// Configuration for which metrics are enabled
#[derive(Debug, Deserialize, Clone)]
pub struct MetricsEnabled {
    // Node metrics
    #[serde(default = "default_true")]
    pub node_info: bool,
    #[serde(default = "default_true")]
    pub node_uptime: bool,

    // CPU metrics
    #[serde(default = "default_true")]
    pub cpu_cores: bool,
    #[serde(default = "default_true")]
    pub cpu_threads: bool,
    #[serde(default = "default_true")]
    pub cpu_usage: bool,
    #[serde(default = "default_true")]
    pub cpu_used_cores: bool,

    // Memory metrics
    #[serde(default = "default_true")]
    pub memory_total: bool,
    #[serde(default = "default_true")]
    pub memory_used: bool,
    #[serde(default = "default_true")]
    pub memory_available: bool,
    #[serde(default = "default_true")]
    pub memory_usage: bool,

    // GPU metrics
    #[serde(default = "default_true")]
    pub gpu_count: bool,
    #[serde(default = "default_true")]
    pub gpu_used_count: bool,
    #[serde(default = "default_true")]
    pub gpu_type_count: bool,
    #[serde(default = "default_true")]
    pub gpu_memory_total: bool,
    #[serde(default = "default_true")]
    pub gpu_memory_used: bool,
    #[serde(default = "default_true")]
    pub gpu_memory_free: bool,
    #[serde(default = "default_true")]
    pub gpu_utilization: bool,
    #[serde(default = "default_true")]
    pub gpu_temperature: bool,
    #[serde(default = "default_true")]
    pub gpu_power_draw: bool,
    #[serde(default = "default_true")]
    pub gpu_power_limit: bool,
}

fn default_true() -> bool {
    true
}

impl Default for MetricsEnabled {
    fn default() -> Self {
        Self {
            node_info: true,
            node_uptime: true,
            cpu_cores: true,
            cpu_threads: true,
            cpu_usage: true,
            cpu_used_cores: true,
            memory_total: true,
            memory_used: true,
            memory_available: true,
            memory_usage: true,
            gpu_count: true,
            gpu_used_count: true,
            gpu_type_count: true,
            gpu_memory_total: true,
            gpu_memory_used: true,
            gpu_memory_free: true,
            gpu_utilization: true,
            gpu_temperature: true,
            gpu_power_draw: true,
            gpu_power_limit: true,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerSettings {
                host: "0.0.0.0".to_string(),
                port: 8080,
            },
            metrics: MetricsSettings {
                collect_interval_secs: 15,
                enabled: MetricsEnabled::default(),
            },
        }
    }
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        let config = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name("config/local").required(false))
            .add_source(Environment::with_prefix("APP").separator("__"))
            .build()?;

        let settings: Settings = config.try_deserialize().unwrap_or_default();
        Ok(settings)
    }
}
