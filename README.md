# System Info Exporter

一个用于采集物理节点硬件信息的 Prometheus Exporter，使用 Rust 编写。支持 CPU、内存和 NVIDIA GPU 指标采集。

## 功能特性

- **CPU 信息采集**：核心数、线程数、型号、使用率
- **内存信息采集**：总量、已用、可用、使用率
- **GPU 信息采集**：通过 `nvidia-smi` 获取 NVIDIA GPU 详细信息
  - GPU 数量及型号统计
  - 显存（总量/已用/可用）
  - 利用率、温度、功耗
  - **数据缓存机制**：防止 nvidia-smi 超时或失败时数据丢失
- **Prometheus 格式输出**：所有指标使用 `hw_` 前缀
- **轻量高效**：Rust 编写，资源占用低（~200m CPU，~256Mi 内存）
- **混合集群支持**：同时兼容 GPU 节点和纯 CPU 节点
- **自定义指标**：支持配置文件启用/禁用特定指标

## 快速开始

### 本地运行

```bash
# 构建
make build

# 运行
make run

# 测试
curl http://localhost:8080/health
curl http://localhost:8080/metrics
```

### Docker 运行

```bash
# 构建镜像
make docker-build

# 运行容器（需要 NVIDIA Container Toolkit）
docker run --rm -p 8080:8080 --gpus all system-info-exporter:latest
```

### Kubernetes 部署

```bash
kubectl apply -f deploy/kubernetes/
```

详细部署说明请参考 [deploy/kubernetes/README.md](deploy/kubernetes/README.md)

## API 端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/health` | GET | 健康检查 |
| `/healthz` | GET | 健康检查（K8s Liveness） |
| `/ready` | GET | 就绪检查（K8s Readiness） |
| `/metrics` | GET | Prometheus 格式指标 |
| `/metrics/json` | GET | JSON 格式指标（旧版兼容） |
| `/node` | GET | 完整节点信息（JSON） |

## Prometheus 指标详解

所有指标均使用 `hw_` 前缀，以下详细说明每个指标的获取方式和计算方法。

### 节点信息指标

| 指标名 | 类型 | 标签 | 说明 | 数据来源 |
|--------|------|------|------|----------|
| `hw_node_info` | gauge | node, os, os_version, kernel, cpu_model | 节点基本信息 | 见下方详解 |
| `hw_node_uptime_seconds` | counter | node | 节点运行时间（秒） | `sysinfo::System::uptime()` |

**`hw_node_info` 标签详解：**

| 标签 | 数据来源 | 说明 |
|------|----------|------|
| `node` | 环境变量 `NODE_NAME`，回退到 `sysinfo::System::host_name()` | K8s 节点名 |
| `os` | `/host/etc/os-release` 的 `NAME` 字段，回退到 `sysinfo::System::name()` | 操作系统名称 |
| `os_version` | `/host/etc/os-release` 的 `VERSION_ID` 字段，回退到 `sysinfo::System::os_version()` | 操作系统版本 |
| `kernel` | `sysinfo::System::kernel_version()` | 内核版本 |
| `cpu_model` | `sysinfo::System::cpus()[0].brand()` | CPU 型号 |

### CPU 指标

| 指标名 | 类型 | 标签 | 说明 | 数据来源 |
|--------|------|------|------|----------|
| `hw_cpu_cores` | gauge | node | 物理核心数 | `sysinfo::System::physical_core_count()` |
| `hw_cpu_threads` | gauge | node | 逻辑线程数 | `sysinfo::System::cpus().len()` |
| `hw_cpu_usage_percent` | gauge | node | CPU 使用率（%） | `sysinfo::System::global_cpu_usage()` |
| `hw_cpu_used_cores` | gauge | node | 使用的 CPU 核心数 | 计算: `(usage_percent / 100) * threads` |

### 内存指标

| 指标名 | 类型 | 标签 | 说明 | 数据来源 |
|--------|------|------|------|----------|
| `hw_memory_total_bytes` | gauge | node | 内存总量（字节） | `sysinfo::System::total_memory()` |
| `hw_memory_used_bytes` | gauge | node | 已用内存（字节） | `sysinfo::System::used_memory()` |
| `hw_memory_available_bytes` | gauge | node | 可用内存（字节） | `sysinfo::System::available_memory()` |
| `hw_memory_usage_percent` | gauge | node | 内存使用率（%） | 计算: `(used / total) * 100` |

### GPU 指标

GPU 指标仅在检测到 NVIDIA GPU 的节点上输出。

#### GPU 检测条件

1. 检查 `/host/proc/driver/nvidia/version` 文件是否存在
2. 查找 `nvidia-smi` 二进制文件（按顺序）：
   - `/usr/bin/nvidia-smi` (NVIDIA Container Toolkit 注入)
   - `/usr/local/bin/nvidia-smi`
   - `/host/usr/bin/nvidia-smi` (宿主机挂载，可能有 glibc 兼容问题)

#### GPU 汇总指标

| 指标名 | 类型 | 标签 | 说明 | 数据来源 |
|--------|------|------|------|----------|
| `hw_gpu_count` | gauge | node | GPU 总数 | `nvidia-smi` 返回的 GPU 数量 |
| `hw_gpu_used_count` | gauge | node | 正在使用的 GPU 数量 | 见下方计算方法 |
| `hw_gpu_type_count` | gauge | node, gpu_type | 按型号统计 GPU 数量 | 按 GPU 名称分组计数 |

**`hw_gpu_used_count` 计算方法：**

```bash
# 查询所有运行计算进程的 GPU UUID
nvidia-smi --query-compute-apps=gpu_uuid --format=csv,noheader
```

统计有计算进程运行的唯一 GPU 数量。这比检测 `memory_used > 0` 更准确，因为空闲 GPU 也会有基础显存占用。

#### GPU 设备详细指标

以下指标为每个 GPU 设备单独输出，包含 `gpu_index`、`gpu_name`、`gpu_uuid` 标签。

| 指标名 | 类型 | 说明 | nvidia-smi 查询字段 |
|--------|------|------|---------------------|
| `hw_gpu_memory_total_bytes` | gauge | GPU 显存总量（字节） | `memory.total` × 1024 × 1024 |
| `hw_gpu_memory_used_bytes` | gauge | GPU 已用显存（字节） | `memory.used` × 1024 × 1024 |
| `hw_gpu_memory_free_bytes` | gauge | GPU 可用显存（字节） | `memory.free` × 1024 × 1024 |
| `hw_gpu_utilization_percent` | gauge | GPU 利用率（%） | `utilization.gpu` |
| `hw_gpu_temperature_celsius` | gauge | GPU 温度（℃） | `temperature.gpu` |
| `hw_gpu_power_draw_watts` | gauge | GPU 功耗（W） | `power.draw` |
| `hw_gpu_power_limit_watts` | gauge | GPU 功率限制（W） | `power.limit` |

**nvidia-smi 查询命令：**

```bash
nvidia-smi --query-gpu=index,name,uuid,memory.total,memory.used,memory.free,utilization.gpu,temperature.gpu,power.draw,power.limit --format=csv,noheader,nounits
```

### 指标示例

```prometheus
# HELP hw_node_info Node hardware information
# TYPE hw_node_info gauge
hw_node_info{node="gpu-node-01",os="Ubuntu",os_version="22.04",kernel="5.15.0",cpu_model="Intel(R) Xeon(R) Gold 6248R"} 1

# HELP hw_node_uptime_seconds Node uptime in seconds
# TYPE hw_node_uptime_seconds counter
hw_node_uptime_seconds{node="gpu-node-01"} 8640000

# HELP hw_cpu_cores Number of physical CPU cores
# TYPE hw_cpu_cores gauge
hw_cpu_cores{node="gpu-node-01"} 48

# HELP hw_cpu_threads Number of CPU threads
# TYPE hw_cpu_threads gauge
hw_cpu_threads{node="gpu-node-01"} 96

# HELP hw_cpu_usage_percent CPU usage percentage
# TYPE hw_cpu_usage_percent gauge
hw_cpu_usage_percent{node="gpu-node-01"} 23.45

# HELP hw_cpu_used_cores Number of CPU cores currently in use
# TYPE hw_cpu_used_cores gauge
hw_cpu_used_cores{node="gpu-node-01"} 22.51

# HELP hw_memory_total_bytes Total memory in bytes
# TYPE hw_memory_total_bytes gauge
hw_memory_total_bytes{node="gpu-node-01"} 270582939648

# HELP hw_memory_used_bytes Used memory in bytes
# TYPE hw_memory_used_bytes gauge
hw_memory_used_bytes{node="gpu-node-01"} 135291469824

# HELP hw_memory_available_bytes Available memory in bytes
# TYPE hw_memory_available_bytes gauge
hw_memory_available_bytes{node="gpu-node-01"} 135291469824

# HELP hw_memory_usage_percent Memory usage percentage
# TYPE hw_memory_usage_percent gauge
hw_memory_usage_percent{node="gpu-node-01"} 50.00

# HELP hw_gpu_count Total number of GPUs per node
# TYPE hw_gpu_count gauge
hw_gpu_count{node="gpu-node-01"} 8

# HELP hw_gpu_used_count Number of GPUs currently in use per node
# TYPE hw_gpu_used_count gauge
hw_gpu_used_count{node="gpu-node-01"} 4

# HELP hw_gpu_type_count Number of GPUs by type per node
# TYPE hw_gpu_type_count gauge
hw_gpu_type_count{node="gpu-node-01",gpu_type="NVIDIA A100-SXM4-80GB"} 8

# HELP hw_gpu_memory_total_bytes GPU total memory in bytes
# TYPE hw_gpu_memory_total_bytes gauge
hw_gpu_memory_total_bytes{node="gpu-node-01",gpu_index="0",gpu_name="NVIDIA A100-SXM4-80GB",gpu_uuid="GPU-12345678-1234-1234-1234-123456789abc"} 85899345920

# HELP hw_gpu_memory_used_bytes GPU used memory in bytes
# TYPE hw_gpu_memory_used_bytes gauge
hw_gpu_memory_used_bytes{node="gpu-node-01",gpu_index="0",gpu_name="NVIDIA A100-SXM4-80GB",gpu_uuid="GPU-12345678-1234-1234-1234-123456789abc"} 42949672960

# HELP hw_gpu_memory_free_bytes GPU free memory in bytes
# TYPE hw_gpu_memory_free_bytes gauge
hw_gpu_memory_free_bytes{node="gpu-node-01",gpu_index="0",gpu_name="NVIDIA A100-SXM4-80GB",gpu_uuid="GPU-12345678-1234-1234-1234-123456789abc"} 42949672960

# HELP hw_gpu_utilization_percent GPU utilization percentage
# TYPE hw_gpu_utilization_percent gauge
hw_gpu_utilization_percent{node="gpu-node-01",gpu_index="0",gpu_name="NVIDIA A100-SXM4-80GB",gpu_uuid="GPU-12345678-1234-1234-1234-123456789abc"} 85

# HELP hw_gpu_temperature_celsius GPU temperature in Celsius
# TYPE hw_gpu_temperature_celsius gauge
hw_gpu_temperature_celsius{node="gpu-node-01",gpu_index="0",gpu_name="NVIDIA A100-SXM4-80GB",gpu_uuid="GPU-12345678-1234-1234-1234-123456789abc"} 65

# HELP hw_gpu_power_draw_watts GPU power draw in watts
# TYPE hw_gpu_power_draw_watts gauge
hw_gpu_power_draw_watts{node="gpu-node-01",gpu_index="0",gpu_name="NVIDIA A100-SXM4-80GB",gpu_uuid="GPU-12345678-1234-1234-1234-123456789abc"} 250

# HELP hw_gpu_power_limit_watts GPU power limit in watts
# TYPE hw_gpu_power_limit_watts gauge
hw_gpu_power_limit_watts{node="gpu-node-01",gpu_index="0",gpu_name="NVIDIA A100-SXM4-80GB",gpu_uuid="GPU-12345678-1234-1234-1234-123456789abc"} 400
```

## 常用 PromQL 查询

### 集群级别聚合

> **重要提示**：`hw_cpu_usage_percent` 和 `hw_memory_usage_percent` 是每个节点的百分比值（0-100）。
> 使用 `sum()` 聚合会得到所有节点百分比的总和（可能超过 100%），这通常不是期望的结果。
> 正确的集群级别百分比计算应使用绝对值相除或使用 `avg()`。

```promql
# ========== CPU 指标 ==========

# 集群 CPU 总核心数
sum(hw_cpu_threads)

# 集群 CPU 已使用核心数
sum(hw_cpu_used_cores)

# 集群 CPU 使用率（正确计算方式）
sum(hw_cpu_used_cores) / sum(hw_cpu_threads) * 100

# 集群平均 CPU 使用率（每节点平均）
avg(hw_cpu_usage_percent)

# ========== 内存指标 ==========

# 集群内存总量
sum(hw_memory_total_bytes)

# 集群已用内存
sum(hw_memory_used_bytes)

# 集群内存使用率（正确计算方式）
sum(hw_memory_used_bytes) / sum(hw_memory_total_bytes) * 100

# 集群平均内存使用率（每节点平均）
avg(hw_memory_usage_percent)

# ========== GPU 指标 ==========

# 集群 GPU 总数
sum(hw_gpu_count)

# 集群正在使用的 GPU 总数
sum(hw_gpu_used_count)

# 集群 GPU 使用率
sum(hw_gpu_used_count) / sum(hw_gpu_count) * 100

# 按 GPU 型号统计集群 GPU 数量
sum by (gpu_type) (hw_gpu_type_count)

# 集群 GPU 显存总量
sum(hw_gpu_memory_total_bytes)

# 集群 GPU 已用显存
sum(hw_gpu_memory_used_bytes)

# 集群 GPU 显存使用率
sum(hw_gpu_memory_used_bytes) / sum(hw_gpu_memory_total_bytes) * 100
```

### 错误用法示例

```promql
# ❌ 错误：sum 百分比会超过 100%
sum(hw_cpu_usage_percent)           # 10个节点各50% = 500%
sum(hw_memory_usage_percent)        # 10个节点各60% = 600%

# ✅ 正确：使用绝对值计算或使用 avg
sum(hw_cpu_used_cores) / sum(hw_cpu_threads) * 100
sum(hw_memory_used_bytes) / sum(hw_memory_total_bytes) * 100
avg(hw_cpu_usage_percent)
avg(hw_memory_usage_percent)
```

### 节点级别查询

```promql
# 某节点的 CPU 使用核心数
hw_cpu_used_cores{node="gpu-node-01"}

# 某节点的 GPU 使用情况
hw_gpu_used_count{node="gpu-node-01"}

# 某节点所有 GPU 的平均利用率
avg by (node) (hw_gpu_utilization_percent{node="gpu-node-01"})

# GPU 温度超过 80℃ 的设备
hw_gpu_temperature_celsius > 80

# 显存使用超过 90% 的 GPU
hw_gpu_memory_used_bytes / hw_gpu_memory_total_bytes * 100 > 90

# CPU 使用率超过 80% 的节点
hw_cpu_usage_percent > 80

# 内存使用率超过 90% 的节点
hw_memory_usage_percent > 90
```

## 配置

### 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `APP__SERVER__HOST` | `0.0.0.0` | 监听地址 |
| `APP__SERVER__PORT` | `8080` | 监听端口 |
| `RUST_LOG` | `info` | 日志级别 |
| `NODE_NAME` | - | K8s 节点名（自动从 fieldRef 获取） |
| `NVIDIA_VISIBLE_DEVICES` | `all` | 可见 GPU 设备 |
| `NVIDIA_DRIVER_CAPABILITIES` | `all` | NVIDIA 驱动能力 |

### 配置文件

配置文件位于 `config/default.toml`：

```toml
[server]
host = "0.0.0.0"
port = 8080

[metrics]
collect_interval_secs = 15

# Metrics collection settings
# Set to false to disable specific metrics
[metrics.enabled]
# Node metrics
node_info = true
node_uptime = true

# CPU metrics
cpu_cores = true
cpu_threads = true
cpu_usage = true

# Memory metrics
memory_total = true
memory_used = true
memory_available = true
memory_usage = true

# GPU metrics (only collected on GPU nodes)
gpu_count = true
gpu_used_count = true
gpu_type_count = true
gpu_memory_total = true
gpu_memory_used = true
gpu_memory_free = true
gpu_utilization = true
gpu_temperature = true
gpu_power_draw = true
gpu_power_limit = true
```

### 自定义指标采集

可以通过配置文件禁用不需要的指标，例如只采集 GPU 相关指标：

```toml
[metrics.enabled]
# 禁用 Node 指标
node_info = false
node_uptime = false

# 禁用 CPU 指标
cpu_cores = false
cpu_threads = false
cpu_usage = false

# 禁用 Memory 指标
memory_total = false
memory_used = false
memory_available = false
memory_usage = false

# 只保留 GPU 指标
gpu_count = true
gpu_used_count = true
gpu_type_count = true
gpu_memory_total = true
gpu_memory_used = true
gpu_memory_free = true
gpu_utilization = true
gpu_temperature = true
gpu_power_draw = true
gpu_power_limit = true
```

## 项目结构

```
system-info-exporter/
├── Cargo.toml              # Rust 项目配置
├── Makefile                # 构建自动化
├── Dockerfile              # 容器构建（Debian glibc 多阶段）
├── entrypoint.sh           # 容器入口脚本
├── VERSION                 # 版本号
├── config/
│   └── default.toml        # 默认配置
├── src/
│   ├── main.rs             # 程序入口
│   ├── config.rs           # 配置加载
│   ├── error.rs            # 错误处理
│   ├── metrics.rs          # 指标采集（CPU/内存/GPU）
│   └── api/
│       ├── mod.rs          # HTTP 服务
│       └── handlers.rs     # 请求处理器
└── deploy/
    └── kubernetes/         # K8s 部署文件
        ├── namespace.yaml
        ├── configmap.yaml
        ├── daemonset.yaml
        ├── service.yaml
        └── README.md
```

## 技术实现

### 依赖库

| 依赖 | 用途 |
|------|------|
| `sysinfo` | 采集 CPU、内存、系统信息 |
| `axum` | HTTP 服务框架 |
| `tokio` | 异步运行时 |
| `serde` | 序列化/反序列化 |
| `config` | 配置文件加载 |
| `tracing` | 日志记录 |
| `lazy_static` | GPU 缓存全局状态 |

### GPU 采集机制

本项目使用 `nvidia-smi` 命令行工具采集 GPU 信息，具有以下特性：

#### 工作流程

```
┌─────────────────────────────────────────────────────────────┐
│                     GPU 指标采集流程                          │
├─────────────────────────────────────────────────────────────┤
│  1. 检查 /host/proc/driver/nvidia/version 是否存在           │
│     └─ 不存在 → 返回空数据（CPU 节点）                        │
│                                                             │
│  2. 查找 nvidia-smi 二进制文件                               │
│     └─ 不存在 → 返回空数据                                   │
│                                                             │
│  3. 执行 nvidia-smi --query-gpu=... (超时 5s)               │
│     ├─ 成功 → 解析数据，更新缓存                              │
│     └─ 失败/超时 → 返回缓存数据（有效期 5 分钟）               │
│                                                             │
│  4. 执行 nvidia-smi --query-compute-apps=gpu_uuid            │
│     └─ 统计正在使用的 GPU 数量                               │
└─────────────────────────────────────────────────────────────┘
```

#### 缓存机制

- **缓存有效期**：5 分钟 (`GPU_CACHE_MAX_AGE_SECS = 300`)
- **命令超时**：5 秒 (`NVIDIA_SMI_TIMEOUT_SECS = 5`)
- **缓存内容**：GPU 设备列表、类型统计、使用数量

当 nvidia-smi 执行失败或超时时，返回缓存数据以保证指标连续性。

#### 为什么选择 nvidia-smi

| 方案 | 优点 | 缺点 |
|------|------|------|
| **nvidia-smi**（当前） | 无 glibc 依赖问题；NVIDIA Container Toolkit 自动注入；兼容性好 | 依赖外部命令；解析开销 |
| nvml-wrapper | 直接调用 NVML API；性能更好 | glibc 版本必须与宿主机一致；容器基础镜像限制严格 |

由于 nvml-wrapper 通过动态链接调用宿主机的 `libnvidia-ml.so`，要求容器内的 glibc 版本与宿主机完全兼容。
这在异构集群中难以保证，因此选择 nvidia-smi 方案以获得更好的兼容性。

### 内存优化

使用 `hostPID: true` 时，`sysinfo` 库默认会采集所有宿主机进程信息，导致内存消耗过高。

**优化方案**：使用选择性刷新，只采集需要的数据：

```rust
// 不使用 System::new_all()，避免采集所有进程
let mut sys = System::new();
sys.refresh_memory();    // 只刷新内存信息
sys.refresh_cpu_all();   // 只刷新 CPU 信息
```

## Kubernetes 部署要点

### 关键配置

```yaml
spec:
  hostPID: true                    # 访问宿主机 PID 命名空间
  containers:
    - name: system-info-exporter
      securityContext:
        privileged: true           # 访问 GPU 设备需要特权模式
        runAsUser: 0
        runAsGroup: 0
      env:
        - name: NVIDIA_VISIBLE_DEVICES
          value: "all"
        - name: NVIDIA_DRIVER_CAPABILITIES
          value: "all"
        - name: NODE_NAME
          valueFrom:
            fieldRef:
              fieldPath: spec.nodeName
      volumeMounts:
        - name: host-dev
          mountPath: /dev          # 挂载宿主机 /dev 以访问 GPU 设备
        - name: host-proc-driver
          mountPath: /host/proc/driver
          readOnly: true
        - name: host-etc-os-release
          mountPath: /host/etc/os-release
          readOnly: true
```

### 重要说明

1. **不要指定 `runtimeClassName: nvidia`**
   - GPU 节点默认使用 nvidia 运行时，会自动注入 nvidia-smi
   - CPU 节点没有 nvidia 运行时，指定会导致 Pod 无法调度

2. **资源限制建议**
   - CPU: 200m
   - Memory: 256Mi
   - 启用 `hostPID: true` 时需要优化内存采集（已实现）

3. **tolerations 配置**
   - 需要容忍 `nvidia.com/gpu` taint 以便在 GPU 节点上调度

## 开发

### 构建要求

- Rust 1.75+

### 运行环境

- NVIDIA 驱动（GPU 节点）
- NVIDIA Container Toolkit（容器环境，GPU 节点）

### 常用命令

```bash
# 格式化代码
make fmt

# 代码检查
make lint

# 运行测试
make test

# 清理构建
make clean

# 构建 Docker 镜像
make docker-build

# 推送镜像
make docker-push
```

## License

MIT
