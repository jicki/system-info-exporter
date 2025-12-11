use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Serialize)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub memory_usage_percent: f32,
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub uptime: u64,
}

pub fn collect() -> SystemMetrics {
    let mut sys = System::new_all();
    sys.refresh_all();

    let memory_total = sys.total_memory();
    let memory_used = sys.used_memory();
    let memory_usage_percent = if memory_total > 0 {
        (memory_used as f32 / memory_total as f32) * 100.0
    } else {
        0.0
    };

    SystemMetrics {
        cpu_usage: sys.global_cpu_usage(),
        memory_total,
        memory_used,
        memory_usage_percent,
        hostname: System::host_name().unwrap_or_else(|| "unknown".to_string()),
        os_name: System::name().unwrap_or_else(|| "unknown".to_string()),
        os_version: System::os_version().unwrap_or_else(|| "unknown".to_string()),
        uptime: System::uptime(),
    }
}
