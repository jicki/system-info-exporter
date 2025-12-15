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

## Prometheus 指标

所有指标均使用 `hw_` 前缀。

### 节点信息

| 指标名 | 类型 | 说明 |
|--------|------|------|
| `hw_node_info` | gauge | 节点基本信息（标签包含 node、os、kernel、cpu_model） |
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
| `hw_gpu_count` | gauge | GPU 总数（仅 GPU 节点输出） |
| `hw_gpu_used_count` | gauge | 正在使用的 GPU 数量（utilization > 0 或 memory_used > 0） |
| `hw_gpu_type_count` | gauge | 按节点和型号统计 GPU 数量 |
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
hw_node_info{node="gpu-node-01",os="Ubuntu",os_version="22.04",kernel="5.15.0",cpu_model="Intel(R) Xeon(R) Gold 6248R"} 1

# HELP hw_cpu_cores Number of physical CPU cores
# TYPE hw_cpu_cores gauge
hw_cpu_cores{node="gpu-node-01"} 48

# HELP hw_memory_total_bytes Total memory in bytes
# TYPE hw_memory_total_bytes gauge
hw_memory_total_bytes{node="gpu-node-01"} 270582939648

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
hw_gpu_memory_total_bytes{node="gpu-node-01",gpu_index="0",gpu_name="NVIDIA A100-SXM4-80GB",gpu_uuid="GPU-xxx"} 85899345920
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

## 依赖要求

### 构建环境

- Rust 1.75+

### 运行环境

- NVIDIA 驱动（GPU 节点）
- NVIDIA Container Toolkit（容器环境，GPU 节点）

## GPU 采集机制

本项目使用 `nvidia-smi` 命令行工具采集 GPU 信息，具有以下特性：

### 工作原理

1. **GPU 硬件检测**：检查 `/host/proc/driver/nvidia/version` 判断节点是否有 NVIDIA GPU
2. **nvidia-smi 查找**：优先使用 NVIDIA Container Toolkit 注入的 `/usr/bin/nvidia-smi`
3. **超时保护**：nvidia-smi 执行超时设置为 5 秒，防止命令卡住阻塞整个服务
4. **数据缓存**：缓存成功获取的 GPU 数据，有效期 5 分钟
   - nvidia-smi 失败时返回缓存数据，保证指标连续性
   - 避免因 GPU 状态异常导致监控数据中断

### 为什么选择 nvidia-smi

| 方案 | 优点 | 缺点 |
|------|------|------|
| **nvidia-smi**（当前） | 无 glibc 依赖问题；NVIDIA Container Toolkit 自动注入；兼容性好 | 依赖外部命令；解析开销 |
| nvml-wrapper | 直接调用 NVML API；性能更好 | glibc 版本必须与宿主机一致；容器基础镜像限制严格 |

由于 nvml-wrapper 通过动态链接调用宿主机的 `libnvidia-ml.so`，要求容器内的 glibc 版本与宿主机完全兼容。
这在异构集群中难以保证，因此选择 nvidia-smi 方案以获得更好的兼容性。

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
