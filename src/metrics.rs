use crate::config::MetricsEnabled;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use sysinfo::System;
use tracing::{info, warn};

/// Timeout for nvidia-smi command execution (in seconds)
const NVIDIA_SMI_TIMEOUT_SECS: u64 = 5;

/// Maximum age of cached GPU data before it's considered stale (in seconds)
/// If nvidia-smi fails and cache is older than this, we still return cached data
/// but log a warning
const GPU_CACHE_MAX_AGE_SECS: u64 = 300; // 5 minutes

/// Path to nvidia-smi binary
/// Prefer container paths (injected by NVIDIA Container Toolkit) over host-mounted paths
/// Using host-mounted binaries causes glibc version mismatch issues
const NVIDIA_SMI_PATHS: &[&str] = &[
    "/usr/bin/nvidia-smi",           // Injected by NVIDIA Container Toolkit
    "/usr/local/bin/nvidia-smi",     // Alternative container path
    "/host/usr/bin/nvidia-smi",      // Host-mounted fallback (may not work due to glibc mismatch)
];

/// Cached GPU information to prevent data loss when nvidia-smi hangs or fails
struct GpuCache {
    devices: Vec<GpuInfo>,
    type_counts: HashMap<String, u32>,
    used_count: usize,
    last_update: Instant,
    last_success: bool,
}

impl Default for GpuCache {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
            type_counts: HashMap::new(),
            used_count: 0,
            last_update: Instant::now(),
            last_success: false,
        }
    }
}

lazy_static::lazy_static! {
    static ref GPU_CACHE: RwLock<GpuCache> = RwLock::new(GpuCache::default());
    /// Persistent System object for accurate CPU usage calculation
    /// sysinfo requires multiple refreshes to calculate CPU usage delta
    static ref SYSTEM: RwLock<System> = RwLock::new(System::new());
}

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
    pub cpu_used_cores: f32,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_available_bytes: u64,
    pub memory_usage_percent: f32,
    pub gpu_count: usize,
    pub gpu_used_count: usize,
    pub gpu_devices: Vec<GpuInfo>,
    pub gpu_type_counts: HashMap<String, u32>,
}

impl NodeMetrics {
    pub fn collect() -> Self {
        // Use persistent System object for accurate CPU usage calculation
        // sysinfo calculates CPU usage by comparing current vs previous refresh
        let mut sys = SYSTEM.write().unwrap();
        sys.refresh_memory();
        sys.refresh_cpu_all();

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

        // Calculate CPU used cores: (usage_percent / 100) * total_threads
        let cpu_threads = sys.cpus().len();
        let cpu_usage_percent = sys.global_cpu_usage();
        let cpu_used_cores = (cpu_usage_percent / 100.0) * cpu_threads as f32;

        let (gpu_devices, gpu_type_counts, gpu_used_count) = collect_gpu_info();

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
            cpu_threads,
            cpu_model,
            cpu_usage_percent,
            cpu_used_cores,
            memory_total_bytes: memory_total,
            memory_used_bytes: memory_used,
            memory_available_bytes: memory_available,
            memory_usage_percent,
            gpu_count: gpu_devices.len(),
            gpu_used_count,
            gpu_devices: gpu_devices.clone(),
            gpu_type_counts,
        }
    }

    pub fn to_prometheus(&self, enabled: &MetricsEnabled) -> String {
        let mut output = String::new();
        let node = &self.node;

        // Node info
        if enabled.node_info {
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
        }

        // Uptime
        if enabled.node_uptime {
            output.push_str("# HELP hw_node_uptime_seconds Node uptime in seconds\n");
            output.push_str("# TYPE hw_node_uptime_seconds counter\n");
            output.push_str(&format!(
                "hw_node_uptime_seconds{{node=\"{}\"}} {}\n",
                node, self.uptime_secs
            ));
        }

        // CPU
        if enabled.cpu_cores {
            output.push_str("# HELP hw_cpu_cores Number of physical CPU cores\n");
            output.push_str("# TYPE hw_cpu_cores gauge\n");
            output.push_str(&format!(
                "hw_cpu_cores{{node=\"{}\"}} {}\n",
                node, self.cpu_cores
            ));
        }

        if enabled.cpu_threads {
            output.push_str("# HELP hw_cpu_threads Number of CPU threads\n");
            output.push_str("# TYPE hw_cpu_threads gauge\n");
            output.push_str(&format!(
                "hw_cpu_threads{{node=\"{}\"}} {}\n",
                node, self.cpu_threads
            ));
        }

        if enabled.cpu_usage {
            output.push_str("# HELP hw_cpu_usage_percent CPU usage percentage\n");
            output.push_str("# TYPE hw_cpu_usage_percent gauge\n");
            output.push_str(&format!(
                "hw_cpu_usage_percent{{node=\"{}\"}} {:.2}\n",
                node, self.cpu_usage_percent
            ));
        }

        if enabled.cpu_used_cores {
            // CPU used cores: calculated as (usage_percent / 100) * total_threads
            output.push_str("# HELP hw_cpu_used_cores Number of CPU cores currently in use\n");
            output.push_str("# TYPE hw_cpu_used_cores gauge\n");
            output.push_str(&format!(
                "hw_cpu_used_cores{{node=\"{}\"}} {:.2}\n",
                node, self.cpu_used_cores
            ));
        }

        // Memory
        if enabled.memory_total {
            output.push_str("# HELP hw_memory_total_bytes Total memory in bytes\n");
            output.push_str("# TYPE hw_memory_total_bytes gauge\n");
            output.push_str(&format!(
                "hw_memory_total_bytes{{node=\"{}\"}} {}\n",
                node, self.memory_total_bytes
            ));
        }

        if enabled.memory_used {
            output.push_str("# HELP hw_memory_used_bytes Used memory in bytes\n");
            output.push_str("# TYPE hw_memory_used_bytes gauge\n");
            output.push_str(&format!(
                "hw_memory_used_bytes{{node=\"{}\"}} {}\n",
                node, self.memory_used_bytes
            ));
        }

        if enabled.memory_available {
            output.push_str("# HELP hw_memory_available_bytes Available memory in bytes\n");
            output.push_str("# TYPE hw_memory_available_bytes gauge\n");
            output.push_str(&format!(
                "hw_memory_available_bytes{{node=\"{}\"}} {}\n",
                node, self.memory_available_bytes
            ));
        }

        if enabled.memory_usage {
            output.push_str("# HELP hw_memory_usage_percent Memory usage percentage\n");
            output.push_str("# TYPE hw_memory_usage_percent gauge\n");
            output.push_str(&format!(
                "hw_memory_usage_percent{{node=\"{}\"}} {:.2}\n",
                node, self.memory_usage_percent
            ));
        }

        // GPU metrics only for nodes with GPUs
        if self.gpu_count > 0 {
            // GPU total count per node
            if enabled.gpu_count {
                output.push_str("# HELP hw_gpu_count Total number of GPUs per node\n");
                output.push_str("# TYPE hw_gpu_count gauge\n");
                output.push_str(&format!(
                    "hw_gpu_count{{node=\"{}\"}} {}\n",
                    node, self.gpu_count
                ));
            }

            // GPU used count (GPUs with running compute processes)
            if enabled.gpu_used_count {
                output.push_str("# HELP hw_gpu_used_count Number of GPUs currently in use per node\n");
                output.push_str("# TYPE hw_gpu_used_count gauge\n");
                output.push_str(&format!(
                    "hw_gpu_used_count{{node=\"{}\"}} {}\n",
                    node, self.gpu_used_count
                ));
            }

            // GPU type counts per node
            if enabled.gpu_type_count {
                output.push_str("# HELP hw_gpu_type_count Number of GPUs by type per node\n");
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
        }

        // GPU device details
        if !self.gpu_devices.is_empty() {
            if enabled.gpu_memory_total {
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
            }

            if enabled.gpu_memory_used {
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
            }

            if enabled.gpu_memory_free {
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
            }

            if enabled.gpu_utilization {
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
            }

            if enabled.gpu_temperature {
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
            }

            if enabled.gpu_power_draw {
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
            }

            if enabled.gpu_power_limit {
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
/// Uses /host/proc/driver/nvidia/version which is mounted from host
fn has_nvidia_gpu() -> bool {
    let path = "/host/proc/driver/nvidia/version";
    let exists = std::path::Path::new(path).exists();
    if exists {
        info!("NVIDIA GPU driver detected at {}", path);
    }
    exists
}

/// Find nvidia-smi binary path
fn find_nvidia_smi() -> Option<&'static str> {
    for path in NVIDIA_SMI_PATHS {
        if std::path::Path::new(path).exists() {
            info!("Found nvidia-smi at {}", path);
            return Some(path);
        }
    }
    warn!("nvidia-smi not found in any of: {:?}", NVIDIA_SMI_PATHS);
    None
}

/// Check if timeout command is available
fn has_timeout_command() -> bool {
    Command::new("which")
        .arg("timeout")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Execute nvidia-smi with timeout protection
/// Returns None if command fails, times out, or nvidia-smi is not available
fn run_nvidia_smi_with_timeout(args: &[&str]) -> Option<String> {
    let nvidia_smi = find_nvidia_smi()?;

    // Check if timeout command exists, otherwise use direct execution
    if !has_timeout_command() {
        info!("timeout command not available, using direct execution");
        return run_nvidia_smi_direct(nvidia_smi, args);
    }

    // Build nvidia-smi command with arguments
    let nvidia_args = args.join(" ");

    // Only set LD_LIBRARY_PATH for host-mounted nvidia-smi
    // Container-injected nvidia-smi (from NVIDIA Container Toolkit) has its own libraries
    let shell_cmd = if nvidia_smi.starts_with("/host/") {
        let ld_library_path = "/host/nvidia-libs:/usr/lib/x86_64-linux-gnu:/usr/lib";
        format!(
            "LD_LIBRARY_PATH={} {} {}",
            ld_library_path,
            nvidia_smi,
            nvidia_args
        )
    } else {
        format!("{} {}", nvidia_smi, nvidia_args)
    };

    // Use timeout command to prevent nvidia-smi from hanging
    let output = Command::new("timeout")
        .arg(format!("{}s", NVIDIA_SMI_TIMEOUT_SECS))
        .arg("sh")
        .arg("-c")
        .arg(&shell_cmd)
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                String::from_utf8(result.stdout).ok()
            } else {
                let exit_code = result.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&result.stderr);
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr_msg = if stderr.is_empty() { "(empty)" } else { stderr.trim() };
                let stdout_msg = if stdout.is_empty() { "(empty)" } else { stdout.trim() };

                if exit_code == 124 {
                    warn!("nvidia-smi command timed out after {}s", NVIDIA_SMI_TIMEOUT_SECS);
                } else {
                    warn!(
                        "nvidia-smi failed with exit code: {}. Path: {}, stdout: {}, stderr: {}",
                        exit_code,
                        nvidia_smi,
                        stdout_msg,
                        stderr_msg
                    );
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

    // Only set LD_LIBRARY_PATH for host-mounted nvidia-smi
    let mut cmd = Command::new(nvidia_smi);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if nvidia_smi.starts_with("/host/") {
        let ld_library_path = "/host/nvidia-libs:/usr/lib/x86_64-linux-gnu:/usr/lib";
        cmd.env("LD_LIBRARY_PATH", ld_library_path);
    }

    let mut child = cmd.spawn().ok()?;

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
                    let output = child.wait_with_output().ok();
                    if let Some(o) = output {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        let stdout = String::from_utf8_lossy(&o.stdout);
                        warn!(
                            "nvidia-smi failed with status: {}, stdout: {}, stderr: {}",
                            status,
                            if stdout.is_empty() { "(empty)" } else { stdout.trim() },
                            if stderr.is_empty() { "(empty)" } else { stderr.trim() }
                        );
                    } else {
                        warn!("nvidia-smi failed with status: {}", status);
                    }
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
/// Uses caching to prevent data loss when nvidia-smi hangs or fails
fn collect_gpu_info() -> (Vec<GpuInfo>, HashMap<String, u32>, usize) {
    // Early return if no NVIDIA GPU hardware detected
    if !has_nvidia_gpu() {
        info!("No NVIDIA GPU hardware detected, skipping GPU metrics collection");
        return (Vec::new(), HashMap::new(), 0);
    }

    // Check if nvidia-smi is available
    if find_nvidia_smi().is_none() {
        info!("nvidia-smi not found, skipping GPU metrics collection");
        return (Vec::new(), HashMap::new(), 0);
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
            let gpu_devices = parse_nvidia_smi_output(&output);

            if gpu_devices.is_empty() {
                warn!("nvidia-smi returned no GPU data, using cached data");
                return get_cached_gpu_info();
            }

            // Count GPU types
            let mut gpu_type_counts: HashMap<String, u32> = HashMap::new();
            for gpu in &gpu_devices {
                *gpu_type_counts.entry(gpu.name.clone()).or_insert(0) += 1;
            }

            // Query GPUs with running compute processes
            let gpu_used_count = get_gpu_used_count(&gpu_devices);

            info!("Collected metrics for {} GPU(s), {} in use", gpu_devices.len(), gpu_used_count);

            // Update cache with successful data
            if let Ok(mut cache) = GPU_CACHE.write() {
                cache.devices = gpu_devices.clone();
                cache.type_counts = gpu_type_counts.clone();
                cache.used_count = gpu_used_count;
                cache.last_update = Instant::now();
                cache.last_success = true;
            }

            (gpu_devices, gpu_type_counts, gpu_used_count)
        }
        None => {
            warn!("Failed to get GPU metrics from nvidia-smi, using cached data");
            get_cached_gpu_info()
        }
    }
}

/// Get cached GPU info, with staleness warning
fn get_cached_gpu_info() -> (Vec<GpuInfo>, HashMap<String, u32>, usize) {
    if let Ok(cache) = GPU_CACHE.read() {
        let age_secs = cache.last_update.elapsed().as_secs();

        if cache.devices.is_empty() {
            warn!("No cached GPU data available");
            return (Vec::new(), HashMap::new(), 0);
        }

        if age_secs > GPU_CACHE_MAX_AGE_SECS {
            warn!(
                "Using stale GPU cache data ({}s old, max {}s)",
                age_secs, GPU_CACHE_MAX_AGE_SECS
            );
        } else {
            info!(
                "Using cached GPU data ({}s old) for {} GPU(s)",
                age_secs,
                cache.devices.len()
            );
        }

        (cache.devices.clone(), cache.type_counts.clone(), cache.used_count)
    } else {
        warn!("Failed to read GPU cache");
        (Vec::new(), HashMap::new(), 0)
    }
}

/// Get count of GPUs with running compute processes
/// Uses nvidia-smi --query-compute-apps to detect GPUs with active processes
fn get_gpu_used_count(gpu_devices: &[GpuInfo]) -> usize {
    use std::collections::HashSet;

    // Query compute processes to find which GPUs have running processes
    let query_args = [
        "--query-compute-apps=gpu_uuid",
        "--format=csv,noheader",
    ];

    match run_nvidia_smi_with_timeout(&query_args) {
        Some(output) => {
            // Collect unique GPU UUIDs that have compute processes
            let used_uuids: HashSet<String> = output
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|uuid| !uuid.is_empty())
                .collect();

            // Count how many of our GPUs have processes
            let count = gpu_devices
                .iter()
                .filter(|gpu| used_uuids.contains(&gpu.uuid))
                .count();

            count
        }
        None => {
            warn!("Failed to query compute apps, returning 0 for gpu_used_count");
            0
        }
    }
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
