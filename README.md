# System Info Exporter

一个用于采集物理节点硬件信息的 Prometheus Exporter，使用 Rust 编写。

## 功能特性

- **CPU 信息采集**：核心数、线程数、型号、使用率
- **内存信息采集**：总量、已用、可用、使用率
- **GPU 信息采集**：通过 NVML 获取 NVIDIA GPU 详细信息
  - GPU 数量及型号统计
  - 显存（总量/已用/可用）
  - 利用率、温度、功耗
- **Prometheus 格式输出**：所有指标使用 `hw_` 前缀
- **轻量高效**：Rust 编写，资源占用低

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
docker run --rm -p 8080:8080 --gpus all system-info-exporter:0.1.0
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
| `/healthz` | GET | 健康检查（K8s） |
| `/ready` | GET | 就绪检查 |
| `/metrics` | GET | Prometheus 格式指标 |
| `/metrics/json` | GET | JSON 格式指标（旧版兼容） |
| `/node` | GET | 完整节点信息（JSON） |

## Prometheus 指标

所有指标均使用 `hw_` 前缀。

### 节点信息

| 指标名 | 类型 | 说明 |
|--------|------|------|
| `hw_node_info` | gauge | 节点基本信息（标签包含 hostname、os、kernel、cpu_model） |
| `hw_node_uptime_seconds` | counter | 节点运行时间（秒） |

### CPU 指标

| 指标名 | 类型 | 说明 |
|--------|------|------|
| `hw_cpu_cores` | gauge | 物理核心数 |
| `hw_cpu_threads` | gauge | 逻辑线程数 |
| `hw_cpu_usage_percent` | gauge | CPU 使用率（%） |

### 内存指标

| 指标名 | 类型 | 说明 |
|--------|------|------|
| `hw_memory_total_bytes` | gauge | 内存总量（字节） |
| `hw_memory_used_bytes` | gauge | 已用内存（字节） |
| `hw_memory_available_bytes` | gauge | 可用内存（字节） |
| `hw_memory_usage_percent` | gauge | 内存使用率（%） |

### GPU 指标

| 指标名 | 类型 | 说明 |
|--------|------|------|
| `hw_gpu_count` | gauge | GPU 总数 |
| `hw_gpu_type_count` | gauge | 按型号统计 GPU 数量 |
| `hw_gpu_memory_total_bytes` | gauge | GPU 显存总量（字节） |
| `hw_gpu_memory_used_bytes` | gauge | GPU 已用显存（字节） |
| `hw_gpu_memory_free_bytes` | gauge | GPU 可用显存（字节） |
| `hw_gpu_utilization_percent` | gauge | GPU 利用率（%） |
| `hw_gpu_temperature_celsius` | gauge | GPU 温度（℃） |
| `hw_gpu_power_draw_watts` | gauge | GPU 功耗（W） |
| `hw_gpu_power_limit_watts` | gauge | GPU 功率限制（W） |

### 指标示例

```prometheus
# HELP hw_node_info Node hardware information
# TYPE hw_node_info gauge
hw_node_info{hostname="gpu-node-01",os="Ubuntu",os_version="22.04",kernel="5.15.0",cpu_model="Intel(R) Xeon(R) Gold 6248R"} 1

# HELP hw_cpu_cores Number of physical CPU cores
# TYPE hw_cpu_cores gauge
hw_cpu_cores{hostname="gpu-node-01"} 48

# HELP hw_memory_total_bytes Total memory in bytes
# TYPE hw_memory_total_bytes gauge
hw_memory_total_bytes{hostname="gpu-node-01"} 270582939648

# HELP hw_gpu_count Total number of GPUs
# TYPE hw_gpu_count gauge
hw_gpu_count{hostname="gpu-node-01"} 8

# HELP hw_gpu_type_count Number of GPUs by type
# TYPE hw_gpu_type_count gauge
hw_gpu_type_count{hostname="gpu-node-01",gpu_type="NVIDIA A100-SXM4-80GB"} 8

# HELP hw_gpu_memory_total_bytes GPU total memory in bytes
# TYPE hw_gpu_memory_total_bytes gauge
hw_gpu_memory_total_bytes{hostname="gpu-node-01",gpu_index="0",gpu_name="NVIDIA A100-SXM4-80GB",gpu_uuid="GPU-xxx"} 85899345920
```

## 配置

### 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `APP__SERVER__HOST` | `0.0.0.0` | 监听地址 |
| `APP__SERVER__PORT` | `8080` | 监听端口 |
| `RUST_LOG` | `info` | 日志级别 |

### 配置文件

配置文件位于 `config/default.toml`：

```toml
[server]
host = "0.0.0.0"
port = 8080

[metrics]
collect_interval_secs = 15
```

## 项目结构

```
system-info-exporter/
├── Cargo.toml              # Rust 项目配置
├── Makefile                # 构建自动化
├── Dockerfile              # 容器构建（CUDA 多阶段）
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

## 依赖要求

### 构建环境

- Rust 1.75+
- CUDA Toolkit（用于编译 nvml-wrapper）

### 运行环境

- NVIDIA 驱动
- NVIDIA Container Toolkit（容器环境）
- libnvidia-ml.so（NVML 运行时库）

## GPU 支持说明

本项目使用 [nvml-wrapper](https://crates.io/crates/nvml-wrapper) 通过 NVML（NVIDIA Management Library）采集 GPU 信息。

### 容器中运行

容器需要访问 NVML 库，确保：

1. 宿主机安装 NVIDIA 驱动
2. 使用 NVIDIA Container Toolkit
3. 运行时添加 `--gpus all` 或配置 `NVIDIA_VISIBLE_DEVICES=all`

### Kubernetes 中运行

需要安装以下组件之一：

- [NVIDIA GPU Operator](https://docs.nvidia.com/datacenter/cloud-native/gpu-operator/overview.html)
- [NVIDIA Device Plugin](https://github.com/NVIDIA/k8s-device-plugin)

## 开发

```bash
# 格式化代码
make fmt

# 代码检查
make lint

# 运行测试
make test

# 清理构建
make clean
```

## License

MIT
