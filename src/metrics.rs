use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::time::Duration;
use sysinfo::System;
use tracing::{info, warn};

/// Timeout for nvidia-smi command execution (in seconds)
const NVIDIA_SMI_TIMEOUT_SECS: u64 = 5;

/// Path to nvidia-smi binary
/// NVIDIA Container Runtime automatically injects nvidia-smi at /usr/bin/nvidia-smi
const NVIDIA_SMI_PATHS: &[&str] = &[
    "/usr/bin/nvidia-smi",
    "/usr/local/bin/nvidia-smi",
];

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
    pub node: String,
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

        // Get node name from NODE_NAME env variable, fallback to hostname
        let node = std::env::var("NODE_NAME")
            .ok()
            .or_else(|| System::host_name())
            .unwrap_or_else(|| "unknown".to_string());

        // Get host OS information from mounted /host/etc/os-release
        let (os_name, os_version) = get_host_os_info();

        NodeMetrics {
            hostname: System::host_name().unwrap_or_else(|| "unknown".to_string()),
            node,
            os_name,
            os_version,
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
        let node = &self.node;

        // Node info
        output.push_str("# HELP hw_node_info Node hardware information\n");
        output.push_str("# TYPE hw_node_info gauge\n");
        output.push_str(&format!(
            "hw_node_info{{node=\"{}\",os=\"{}\",os_version=\"{}\",kernel=\"{}\",cpu_model=\"{}\"}} 1\n",
            node,
            self.os_name,
            self.os_version,
            self.kernel_version,
            escape_label_value(&self.cpu_model)
        ));

        // Uptime
        output.push_str("# HELP hw_node_uptime_seconds Node uptime in seconds\n");
        output.push_str("# TYPE hw_node_uptime_seconds counter\n");
        output.push_str(&format!(
            "hw_node_uptime_seconds{{node=\"{}\"}} {}\n",
            node, self.uptime_secs
        ));

        // CPU
        output.push_str("# HELP hw_cpu_cores Number of physical CPU cores\n");
        output.push_str("# TYPE hw_cpu_cores gauge\n");
        output.push_str(&format!(
            "hw_cpu_cores{{node=\"{}\"}} {}\n",
            node, self.cpu_cores
        ));

        output.push_str("# HELP hw_cpu_threads Number of CPU threads\n");
        output.push_str("# TYPE hw_cpu_threads gauge\n");
        output.push_str(&format!(
            "hw_cpu_threads{{node=\"{}\"}} {}\n",
            node, self.cpu_threads
        ));

        output.push_str("# HELP hw_cpu_usage_percent CPU usage percentage\n");
        output.push_str("# TYPE hw_cpu_usage_percent gauge\n");
        output.push_str(&format!(
            "hw_cpu_usage_percent{{node=\"{}\"}} {:.2}\n",
            node, self.cpu_usage_percent
        ));

        // Memory
        output.push_str("# HELP hw_memory_total_bytes Total memory in bytes\n");
        output.push_str("# TYPE hw_memory_total_bytes gauge\n");
        output.push_str(&format!(
            "hw_memory_total_bytes{{node=\"{}\"}} {}\n",
            node, self.memory_total_bytes
        ));

        output.push_str("# HELP hw_memory_used_bytes Used memory in bytes\n");
        output.push_str("# TYPE hw_memory_used_bytes gauge\n");
        output.push_str(&format!(
            "hw_memory_used_bytes{{node=\"{}\"}} {}\n",
            node, self.memory_used_bytes
        ));

        output.push_str("# HELP hw_memory_available_bytes Available memory in bytes\n");
        output.push_str("# TYPE hw_memory_available_bytes gauge\n");
        output.push_str(&format!(
            "hw_memory_available_bytes{{node=\"{}\"}} {}\n",
            node, self.memory_available_bytes
        ));

        output.push_str("# HELP hw_memory_usage_percent Memory usage percentage\n");
        output.push_str("# TYPE hw_memory_usage_percent gauge\n");
        output.push_str(&format!(
            "hw_memory_usage_percent{{node=\"{}\"}} {:.2}\n",
            node, self.memory_usage_percent
        ));

        // GPU total count
        output.push_str("# HELP hw_gpu_count Total number of GPUs\n");
        output.push_str("# TYPE hw_gpu_count gauge\n");
        output.push_str(&format!(
            "hw_gpu_count{{node=\"{}\"}} {}\n",
            node, self.gpu_count
        ));

        // GPU type counts
        if !self.gpu_type_counts.is_empty() {
            output.push_str("# HELP hw_gpu_type_count Number of GPUs by type\n");
            output.push_str("# TYPE hw_gpu_type_count gauge\n");
            for (gpu_type, count) in &self.gpu_type_counts {
                output.push_str(&format!(
                    "hw_gpu_type_count{{node=\"{}\",gpu_type=\"{}\"}} {}\n",
                    node,
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
                    "hw_gpu_memory_total_bytes{{node=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    node,
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
                    "hw_gpu_memory_used_bytes{{node=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    node,
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
                    "hw_gpu_memory_free_bytes{{node=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    node,
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
                    "hw_gpu_utilization_percent{{node=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    node,
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
                    "hw_gpu_temperature_celsius{{node=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    node,
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
                    "hw_gpu_power_draw_watts{{node=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    node,
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
                    "hw_gpu_power_limit_watts{{node=\"{}\",gpu_index=\"{}\",gpu_name=\"{}\",gpu_uuid=\"{}\"}} {}\n",
                    node,
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

/// Parse /etc/os-release file to get OS name and version
/// Returns (os_name, os_version)
fn parse_os_release(path: &str) -> Option<(String, String)> {
    let content = fs::read_to_string(path).ok()?;

    let mut name = None;
    let mut version = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("NAME=") {
            name = Some(line[5..].trim_matches('"').to_string());
        } else if line.starts_with("VERSION_ID=") {
            version = Some(line[11..].trim_matches('"').to_string());
        }

        if name.is_some() && version.is_some() {
            break;
        }
    }

    match (name, version) {
        (Some(n), Some(v)) => Some((n, v)),
        _ => None,
    }
}

/// Get host OS information, trying host mount first, then falling back to container OS
fn get_host_os_info() -> (String, String) {
    // Try to read from host mount first
    if let Some((name, version)) = parse_os_release("/host/etc/os-release") {
        return (name, version);
    }

    // Fallback to sysinfo for container OS
    let os_name = System::name().unwrap_or_else(|| "unknown".to_string());
    let os_version = System::os_version().unwrap_or_else(|| "unknown".to_string());

    (os_name, os_version)
}

/// Check if NVIDIA GPU hardware is present
/// Uses /proc/driver/nvidia/version which is mounted from host
fn has_nvidia_gpu() -> bool {
    std::path::Path::new("/proc/driver/nvidia/version").exists()
}

/// Find nvidia-smi binary path
fn find_nvidia_smi() -> Option<&'static str> {
    for path in NVIDIA_SMI_PATHS {
        if std::path::Path::new(path).exists() {
            return Some(path);
        }
    }
    None
}

/// Execute nvidia-smi with timeout protection
/// Returns None if command fails, times out, or nvidia-smi is not available
fn run_nvidia_smi_with_timeout(args: &[&str]) -> Option<String> {
    let nvidia_smi = find_nvidia_smi()?;
    
    // Use timeout command to prevent nvidia-smi from hanging
    // This is more reliable than Rust-side timeout for process hangs
    let output = Command::new("timeout")
        .arg(format!("{}s", NVIDIA_SMI_TIMEOUT_SECS))
        .arg(nvidia_smi)
        .args(args)
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                String::from_utf8(result.stdout).ok()
            } else {
                let exit_code = result.status.code().unwrap_or(-1);
                if exit_code == 124 {
                    warn!("nvidia-smi command timed out after {}s", NVIDIA_SMI_TIMEOUT_SECS);
                } else {
                    warn!("nvidia-smi failed with exit code: {}", exit_code);
                }
                None
            }
        }
        Err(e) => {
            // timeout command might not exist, try direct execution with spawn
            warn!("Failed to run timeout wrapper: {}, trying direct execution", e);
            run_nvidia_smi_direct(nvidia_smi, args)
        }
    }
}

/// Direct nvidia-smi execution without timeout wrapper
/// Used as fallback when timeout command is not available
fn run_nvidia_smi_direct(nvidia_smi: &str, args: &[&str]) -> Option<String> {
    use std::process::Stdio;
    use std::thread;
    
    let mut child = Command::new(nvidia_smi)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let timeout = Duration::from_secs(NVIDIA_SMI_TIMEOUT_SECS);
    let start = std::time::Instant::now();

    // Poll for completion with timeout
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    let output = child.wait_with_output().ok()?;
                    return String::from_utf8(output.stdout).ok();
                } else {
                    warn!("nvidia-smi failed with status: {}", status);
                    return None;
                }
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    warn!("nvidia-smi timed out after {}s, killing process", NVIDIA_SMI_TIMEOUT_SECS);
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                warn!("Failed to wait for nvidia-smi: {}", e);
                return None;
            }
        }
    }
}

/// Parse GPU information from nvidia-smi CSV output
fn parse_nvidia_smi_output(output: &str) -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if fields.len() < 10 {
            warn!("Invalid nvidia-smi output line: {}", line);
            continue;
        }

        // Parse each field, using 0 as default for numeric fields
        let index = fields[0].parse::<u32>().unwrap_or(0);
        let name = fields[1].to_string();
        let uuid = fields[2].to_string();
        let memory_total = parse_mib_value(fields[3]);
        let memory_used = parse_mib_value(fields[4]);
        let memory_free = parse_mib_value(fields[5]);
        let utilization = parse_percent_value(fields[6]);
        let temperature = parse_int_value(fields[7]);
        let power_draw = parse_watts_value(fields[8]);
        let power_limit = parse_watts_value(fields[9]);

        gpus.push(GpuInfo {
            index,
            name,
            uuid,
            memory_total_mb: memory_total,
            memory_used_mb: memory_used,
            memory_free_mb: memory_free,
            utilization_percent: utilization,
            temperature_celsius: temperature,
            power_draw_watts: power_draw,
            power_limit_watts: power_limit,
        });
    }

    gpus
}

/// Parse MiB value (e.g., "24576" or "24576 MiB")
fn parse_mib_value(s: &str) -> u64 {
    let s = s.trim().replace(" MiB", "").replace(" MB", "");
    s.parse::<u64>().unwrap_or(0)
}

/// Parse percentage value (e.g., "45" or "45 %")
fn parse_percent_value(s: &str) -> u32 {
    let s = s.trim().replace(" %", "").replace("%", "");
    // Handle [N/A] or other non-numeric values
    if s.contains("N/A") || s.contains("[") {
        return 0;
    }
    s.parse::<u32>().unwrap_or(0)
}

/// Parse integer value
fn parse_int_value(s: &str) -> u32 {
    let s = s.trim();
    if s.contains("N/A") || s.contains("[") {
        return 0;
    }
    s.parse::<u32>().unwrap_or(0)
}

/// Parse watts value (e.g., "150.00" or "150.00 W")
fn parse_watts_value(s: &str) -> u32 {
    let s = s.trim().replace(" W", "");
    if s.contains("N/A") || s.contains("[") {
        return 0;
    }
    // Parse as float and convert to integer
    s.parse::<f64>().map(|v| v as u32).unwrap_or(0)
}

/// Collect GPU information using nvidia-smi command
fn collect_gpu_info() -> (Vec<GpuInfo>, HashMap<String, u32>) {
    let mut gpu_devices = Vec::new();
    let mut gpu_type_counts: HashMap<String, u32> = HashMap::new();

    // Early return if no NVIDIA GPU hardware detected
    if !has_nvidia_gpu() {
        info!("No NVIDIA GPU hardware detected, skipping GPU metrics collection");
        return (gpu_devices, gpu_type_counts);
    }

    // Check if nvidia-smi is available
    if find_nvidia_smi().is_none() {
        info!("nvidia-smi not found, skipping GPU metrics collection");
        return (gpu_devices, gpu_type_counts);
    }

    // Query GPU information using nvidia-smi
    // Format: index, name, uuid, memory.total, memory.used, memory.free, 
    //         utilization.gpu, temperature.gpu, power.draw, power.limit
    let query_args = [
        "--query-gpu=index,name,uuid,memory.total,memory.used,memory.free,utilization.gpu,temperature.gpu,power.draw,power.limit",
        "--format=csv,noheader,nounits",
    ];

    match run_nvidia_smi_with_timeout(&query_args) {
        Some(output) => {
            gpu_devices = parse_nvidia_smi_output(&output);
            
            // Count GPU types
            for gpu in &gpu_devices {
                *gpu_type_counts.entry(gpu.name.clone()).or_insert(0) += 1;
            }

            if gpu_devices.is_empty() {
                warn!("nvidia-smi returned no GPU data");
            } else {
                info!("Collected metrics for {} GPU(s)", gpu_devices.len());
            }
        }
        None => {
            warn!("Failed to get GPU metrics from nvidia-smi");
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
