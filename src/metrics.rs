use nvml_wrapper::Nvml;
use serde::Serialize;
use std::collections::HashMap;
use sysinfo::System;
use tracing::warn;

#[derive(Debug, Serialize, Clone)]
pub struct GpuInfo {
    pub index: u32,
    pub name: String,
    pub uuid: String,
    pub memory_total_mb: u64,
    pub memory_used_mb: u64,
    pub memory_free_mb: u64,
    pub utilization_percent: u32,
    pub temperature_celsius: u32,
    pub power_draw_watts: u32,
    pub power_limit_watts: u32,
}

#[derive(Debug, Serialize, Clone)]
pub struct NodeMetrics {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub uptime_secs: u64,
    pub cpu_cores: usize,
    pub cpu_threads: usize,
    pub cpu_model: String,
    pub cpu_usage_percent: f32,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_available_bytes: u64,
    pub memory_usage_percent: f32,
    pub gpu_count: usize,
    pub gpu_devices: Vec<GpuInfo>,
    pub gpu_type_counts: HashMap<String, u32>,
}

impl NodeMetrics {
    pub fn collect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let memory_total = sys.total_memory();
        let memory_used = sys.used_memory();
        let memory_available = sys.available_memory();
        let memory_usage_percent = if memory_total > 0 {
            (memory_used as f32 / memory_total as f32) * 100.0
        } else {
            0.0
        };

        let cpu_model = sys
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let (gpu_devices, gpu_type_counts) = collect_gpu_info();

        NodeMetrics {
            hostname: System::host_name().unwrap_or_else(|| "unknown".to_string()),
            os_name: System::name().unwrap_or_else(|| "unknown".to_string()),
            os_version: System::os_version().unwrap_or_else(|| "unknown".to_string()),
            kernel_version: System::kernel_version().unwrap_or_else(|| "unknown".to_string()),
            uptime_secs: System::uptime(),
            cpu_cores: sys.physical_core_count().unwrap_or(0),
            cpu_threads: sys.cpus().len(),
            cpu_model,
            cpu_usage_percent: sys.global_cpu_usage(),
            memory_total_bytes: memory_total,
            memory_used_bytes: memory_used,
            memory_available_bytes: memory_available,
            memory_usage_percent,
            gpu_count: gpu_devices.len(),
            gpu_devices: gpu_devices.clone(),
            gpu_type_counts,
        }
    }

    pub fn to_prometheus(&self) -> String {
        let mut output = String::new();
        let hostname = &self.hostname;

        // Node info
        output.push_str("# HELP hw_node_info Node hardware information\n");
        output.push_str("# TYPE hw_node_info gauge\n");
        output.push_str(&format!(
            "hw_node_info{{hostname=\"{}\",os=\"{}\",os_version=\"{}\",kernel=\"{}\",cpu_model=\"{}\"}} 1\n",
            hostname,
            self.os_name,
            self.os_version,
            self.kernel_version,
            escape_label_value(&self.cpu_model)
        ));

        // Uptime
        output.push_str("# HELP hw_node_uptime_seconds Node uptime in seconds\n");
        output.push_str("# TYPE hw_node_uptime_seconds counter\n");
        output.push_str(&format!(
            "hw_node_uptime_seconds{{hostname=\"{}\"}} {}\n",
            hostname, self.uptime_secs
        ));

        // CPU
        output.push_str("# HELP hw_cpu_cores Number of physical CPU cores\n");
        output.push_str("# TYPE hw_cpu_cores gauge\n");
        output.push_str(&format!(
            "hw_cpu_cores{{hostname=\"{}\"}} {}\n",
            hostname, self.cpu_cores
        ));

        output.push_str("# HELP hw_cpu_threads Number of CPU threads\n");
        output.push_str("# TYPE hw_cpu_threads gauge\n");
        output.push_str(&format!(
            "hw_cpu_threads{{hostname=\"{}\"}} {}\n",
            hostname, self.cpu_threads
        ));

        output.push_str("# HELP hw_cpu_usage_percent CPU usage percentage\n");
        output.push_str("# TYPE hw_cpu_usage_percent gauge\n");
        output.push_str(&format!(
            "hw_cpu_usage_percent{{hostname=\"{}\"}} {:.2}\n",
            hostname, self.cpu_usage_percent
        ));

        // Memory
        output.push_str("# HELP hw_memory_total_bytes Total memory in bytes\n");
        output.push_str("# TYPE hw_memory_total_bytes gauge\n");
        output.push_str(&format!(
            "hw_memory_total_bytes{{hostname=\"{}\"}} {}\n",
            hostname, self.memory_total_bytes
        ));

        output.push_str("# HELP hw_memory_used_bytes Used memory in bytes\n");
        output.push_str("# TYPE hw_memory_used_bytes gauge\n");
        output.push_str(&format!(
            "hw_memory_used_bytes{{hostname=\"{}\"}} {}\n",
            hostname, self.memory_used_bytes
        ));

        output.push_str("# HELP hw_memory_available_bytes Available memory in bytes\n");
        output.push_str("# TYPE hw_memory_available_bytes gauge\n");
        output.push_str(&format!(
            "hw_memory_available_bytes{{hostname=\"{}\"}} {}\n",
            hostname, self.memory_available_bytes
        ));

        output.push_str("# HELP hw_memory_usage_percent Memory usage percentage\n");
        output.push_str("# TYPE hw_memory_usage_percent gauge\n");
        output.push_str(&format!(
            "hw_memory_usage_percent{{hostname=\"{}\"}} {:.2}\n",
            hostname, self.memory_usage_percent
        ));

        // GPU total count
        output.push_str("# HELP hw_gpu_count Total number of GPUs\n");
        output.push_str("# TYPE hw_gpu_count gauge\n");
        output.push_str(&format!(
            "hw_gpu_count{{hostname=\"{}\"}} {}\n",
            hostname, self.gpu_count
        ));

        // GPU type counts
        if !self.gpu_type_counts.is_empty() {
            output.push_str("# HELP hw_gpu_type_count Number of GPUs by type\n");
            output.push_str("# TYPE hw_gpu_type_count gauge\n");
            for (gpu_type, count) in &self.gpu_type_counts {
                output.push_str(&format!(
                    "hw_gpu_type_count{{hostname=\"{}\",gpu_type=\"{}\"}} {}\n",
                    hostname,
                    escape_label_value(gpu_type),
                    count
                ));
            }
        }

        // GPU device details
        if !self.gpu_devices.is_empty() {
            output.push_str("# HELP hw_gpu_memory_total_bytes GPU total memory in bytes\n");
            output.push_str("# TYPE hw_gpu_memory_total_bytes gauge\n");
            for gpu in &self.gpu_devices {
                output.push_str(&format!(
                    "hw_gpu_memory_total_bytes{{hostname=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    hostname,
                    gpu.index,
                    escape_label_value(&gpu.name),
                    gpu.uuid,
                    gpu.memory_total_mb as u64 * 1024 * 1024
                ));
            }

            output.push_str("# HELP hw_gpu_memory_used_bytes GPU used memory in bytes\n");
            output.push_str("# TYPE hw_gpu_memory_used_bytes gauge\n");
            for gpu in &self.gpu_devices {
                output.push_str(&format!(
                    "hw_gpu_memory_used_bytes{{hostname=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    hostname,
                    gpu.index,
                    escape_label_value(&gpu.name),
                    gpu.uuid,
                    gpu.memory_used_mb as u64 * 1024 * 1024
                ));
            }

            output.push_str("# HELP hw_gpu_memory_free_bytes GPU free memory in bytes\n");
            output.push_str("# TYPE hw_gpu_memory_free_bytes gauge\n");
            for gpu in &self.gpu_devices {
                output.push_str(&format!(
                    "hw_gpu_memory_free_bytes{{hostname=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    hostname,
                    gpu.index,
                    escape_label_value(&gpu.name),
                    gpu.uuid,
                    gpu.memory_free_mb as u64 * 1024 * 1024
                ));
            }

            output.push_str("# HELP hw_gpu_utilization_percent GPU utilization percentage\n");
            output.push_str("# TYPE hw_gpu_utilization_percent gauge\n");
            for gpu in &self.gpu_devices {
                output.push_str(&format!(
                    "hw_gpu_utilization_percent{{hostname=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    hostname,
                    gpu.index,
                    escape_label_value(&gpu.name),
                    gpu.uuid,
                    gpu.utilization_percent
                ));
            }

            output.push_str("# HELP hw_gpu_temperature_celsius GPU temperature in Celsius\n");
            output.push_str("# TYPE hw_gpu_temperature_celsius gauge\n");
            for gpu in &self.gpu_devices {
                output.push_str(&format!(
                    "hw_gpu_temperature_celsius{{hostname=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    hostname,
                    gpu.index,
                    escape_label_value(&gpu.name),
                    gpu.uuid,
                    gpu.temperature_celsius
                ));
            }

            output.push_str("# HELP hw_gpu_power_draw_watts GPU power draw in watts\n");
            output.push_str("# TYPE hw_gpu_power_draw_watts gauge\n");
            for gpu in &self.gpu_devices {
                output.push_str(&format!(
                    "hw_gpu_power_draw_watts{{hostname=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    hostname,
                    gpu.index,
                    escape_label_value(&gpu.name),
                    gpu.uuid,
                    gpu.power_draw_watts
                ));
            }

            output.push_str("# HELP hw_gpu_power_limit_watts GPU power limit in watts\n");
            output.push_str("# TYPE hw_gpu_power_limit_watts gauge\n");
            for gpu in &self.gpu_devices {
                output.push_str(&format!(
                    "hw_gpu_power_limit_watts{{hostname=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    hostname,
                    gpu.index,
                    escape_label_value(&gpu.name),
                    gpu.uuid,
                    gpu.power_limit_watts
                ));
            }
        }

        output
    }
}

fn escape_label_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn collect_gpu_info() -> (Vec<GpuInfo>, HashMap<String, u32>) {
    let mut gpu_devices = Vec::new();
    let mut gpu_type_counts: HashMap<String, u32> = HashMap::new();

    match Nvml::init() {
        Ok(nvml) => {
            let device_count = nvml.device_count().unwrap_or(0);

            for i in 0..device_count {
                match nvml.device_by_index(i) {
                    Ok(device) => {
                        let name = device.name().unwrap_or_else(|_| "Unknown GPU".to_string());
                        let uuid = device.uuid().unwrap_or_else(|_| format!("GPU-{}", i));

                        let memory_info = device.memory_info().ok();
                        let utilization = device.utilization_rates().ok();
                        let temperature = device
                            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                            .ok();
                        let power_draw = device.power_usage().ok();
                        let power_limit = device.enforced_power_limit().ok();

                        let gpu = GpuInfo {
                            index: i,
                            name: name.clone(),
                            uuid,
                            memory_total_mb: memory_info
                                .as_ref()
                                .map(|m| m.total / (1024 * 1024))
                                .unwrap_or(0),
                            memory_used_mb: memory_info
                                .as_ref()
                                .map(|m| m.used / (1024 * 1024))
                                .unwrap_or(0),
                            memory_free_mb: memory_info
                                .as_ref()
                                .map(|m| m.free / (1024 * 1024))
                                .unwrap_or(0),
                            utilization_percent: utilization.map(|u| u.gpu).unwrap_or(0),
                            temperature_celsius: temperature.unwrap_or(0),
                            power_draw_watts: power_draw.map(|p| p / 1000).unwrap_or(0), // mW to W
                            power_limit_watts: power_limit.map(|p| p / 1000).unwrap_or(0), // mW to W
                        };

                        *gpu_type_counts.entry(name).or_insert(0) += 1;
                        gpu_devices.push(gpu);
                    }
                    Err(e) => {
                        warn!("Failed to get GPU device {}: {}", i, e);
                    }
                }
            }
        }
        Err(e) => {
            warn!(
                "Failed to initialize NVML: {}. GPU metrics will be unavailable.",
                e
            );
        }
    }

    (gpu_devices, gpu_type_counts)
}

// Keep the old struct for backward compatibility
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
    let node = NodeMetrics::collect();
    SystemMetrics {
        cpu_usage: node.cpu_usage_percent,
        memory_total: node.memory_total_bytes,
        memory_used: node.memory_used_bytes,
        memory_usage_percent: node.memory_usage_percent,
        hostname: node.hostname,
        os_name: node.os_name,
        os_version: node.os_version,
        uptime: node.uptime_secs,
    }
}
