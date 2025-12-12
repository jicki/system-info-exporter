# Kubernetes 部署指南

本目录包含在 Kubernetes 集群中部署 system-info-exporter 的所有资源文件。

## 📁 文件说明

| 文件 | 说明 |
|-----|------|
| `namespace.yaml` | 命名空间定义 |
| `configmap.yaml` | 应用配置 |
| `daemonset.yaml` | DaemonSet 配置（支持 GPU + CPU 混合节点） |
| `service.yaml` | Service 定义 |
| `servicemonitor.yaml` | Prometheus ServiceMonitor（可选） |

## 🚀 快速部署

### 一键部署所有资源

```bash
kubectl apply -f deploy/kubernetes/
```

### 分步部署

```bash
# 1. 创建命名空间
kubectl apply -f namespace.yaml

# 2. 创建配置
kubectl apply -f configmap.yaml

# 3. 部署 DaemonSet
kubectl apply -f daemonset.yaml

# 4. 创建 Service
kubectl apply -f service.yaml

# 5. 可选：如果使用 Prometheus Operator
kubectl apply -f servicemonitor.yaml
```

## 📋 前置条件

### GPU 节点要求（可选）

如果集群中有 GPU 节点：

1. **安装 NVIDIA 驱动**
   ```bash
   # 验证驱动安装
   nvidia-smi
   ```

2. **安装 NVIDIA Container Toolkit**
   
   需要以下组件之一：
   - [NVIDIA GPU Operator](https://docs.nvidia.com/datacenter/cloud-native/gpu-operator/overview.html)（推荐）
   - [NVIDIA Device Plugin](https://github.com/NVIDIA/k8s-device-plugin)

### CPU 节点要求

无特殊要求，标准 Kubernetes 节点即可。

## 🏗️ 工作原理

### 统一 DaemonSet 架构

```
┌─────────────────────────────────────────────────┐
│              Kubernetes 集群                     │
├─────────────────────────────────────────────────┤
│                                                 │
│  GPU 节点                     CPU 节点           │
│  ┌────────────────┐          ┌───────────────┐ │
│  │   Pod          │          │   Pod         │ │
│  │ (auto GPU)     │          │ (CPU only)    │ │
│  ├────────────────┤          ├───────────────┤ │
│  │ • GPU 指标  ✅ │          │ • CPU 指标 ✅ │ │
│  │ • CPU 指标  ✅ │          │ • 内存指标 ✅ │ │
│  │ • 内存指标  ✅ │          │               │ │
│  └────────────────┘          └───────────────┘ │
│         │                           │          │
│         └───────────┬───────────────┘          │
│                     ↓                          │
│            ┌─────────────────┐                 │
│            │     Service     │                 │
│            └─────────────────┘                 │
└─────────────────────────────────────────────────┘
```

### 智能 GPU 检测

1. **GPU 节点**
   - NVIDIA Container Toolkit 通过 `NVIDIA_VISIBLE_DEVICES` 环境变量自动注入 GPU 支持
   - 代码检测到 GPU 设备文件（`/dev/nvidiactl`、`/dev/nvidia0` 等）
   - 初始化 NVML，采集 GPU、CPU、内存全部指标

2. **CPU 节点**
   - 无 NVIDIA Container Toolkit，环境变量被忽略
   - 代码检测不到 GPU 设备文件
   - 跳过 GPU 初始化，只采集 CPU 和内存指标
   - 日志：`INFO: No NVIDIA GPU hardware detected, skipping GPU metrics collection`

## 🔍 验证部署

### 检查 Pod 状态

```bash
# 查看所有 Pod
kubectl get pods -n system-info-exporter -o wide

# 应该在所有节点上都有一个 Pod
```

### 检查日志

```bash
# 查看 GPU 节点日志（应该能看到 GPU 信息）
kubectl logs -n system-info-exporter <gpu-node-pod-name> --tail=50

# 查看 CPU 节点日志（应该看到跳过 GPU 检测）
kubectl logs -n system-info-exporter <cpu-node-pod-name> --tail=50
```

### 测试指标采集

```bash
# 端口转发
kubectl port-forward -n system-info-exporter svc/system-info-exporter 8080:80

# 测试指标
curl http://localhost:8080/metrics | grep hw_gpu_count
curl http://localhost:8080/metrics | grep hw_cpu_cores
```

## 📊 预期结果

### GPU 节点日志

```json
{"timestamp":"2025-12-12T06:30:00Z","level":"INFO","message":"Starting system-info-exporter"}
{"timestamp":"2025-12-12T06:30:01Z","level":"INFO","message":"Collecting metrics..."}
```

### CPU 节点日志

```json
{"timestamp":"2025-12-12T06:30:00Z","level":"INFO","message":"Starting system-info-exporter"}
{"timestamp":"2025-12-12T06:30:00Z","level":"INFO","message":"No NVIDIA GPU hardware detected, skipping GPU metrics collection"}
```

### 指标示例

#### GPU 节点

```prometheus
hw_gpu_count{node="gpu-node-01"} 8
hw_gpu_memory_total_bytes{node="gpu-node-01",gpu_index="0",...} 85899345920
hw_cpu_cores{node="gpu-node-01"} 48
hw_memory_total_bytes{node="gpu-node-01"} 270582939648
```

#### CPU 节点

```prometheus
hw_gpu_count{node="cpu-node-01"} 0
hw_cpu_cores{node="cpu-node-01"} 16
hw_memory_total_bytes{node="cpu-node-01"} 67108864000
```

## 🔧 配置调整

### 修改资源限制

编辑 `daemonset.yaml` 中的 `resources` 部分：

```yaml
resources:
  requests:
    cpu: 50m
    memory: 64Mi
  limits:
    cpu: 200m
    memory: 128Mi
```

### 修改日志级别

编辑 `daemonset.yaml` 中的环境变量：

```yaml
env:
  - name: RUST_LOG
    value: "debug"  # 可选值: error, warn, info, debug, trace
```

### 修改采集间隔

编辑 `configmap.yaml` 文件：

```yaml
data:
  default.toml: |
    [metrics]
    collect_interval_secs = 15  # 采集间隔（秒）
```

## 🔄 更新部署

### 更新镜像版本

```bash
kubectl set image daemonset/system-info-exporter \
  system-info-exporter=reg.deeproute.ai/deeproute-public/zzh/system-info-exporter:0.1.2 \
  -n system-info-exporter
```

### 重启 Pod

```bash
kubectl rollout restart daemonset/system-info-exporter -n system-info-exporter
```

### 查看滚动更新状态

```bash
kubectl rollout status daemonset/system-info-exporter -n system-info-exporter
```

## 🗑️ 卸载

```bash
# 删除所有资源
kubectl delete -f deploy/kubernetes/

# 或分步删除
kubectl delete -f daemonset.yaml
kubectl delete -f service.yaml
kubectl delete -f configmap.yaml
kubectl delete -f namespace.yaml
```

## ❓ 故障排查

### GPU 节点上仍然报错 NVML 初始化失败

**问题诊断**：

1. **检查 NVIDIA Container Toolkit**
   ```bash
   # 在节点上检查
   docker run --rm --gpus all nvidia/cuda:11.0-base nvidia-smi
   ```

2. **检查 NVIDIA Device Plugin**
   ```bash
   kubectl get pods -n kube-system | grep nvidia
   ```

3. **查看 Pod 详细信息**
   ```bash
   kubectl describe pod -n system-info-exporter <pod-name>
   ```

4. **查看容器日志**
   ```bash
   kubectl logs -n system-info-exporter <pod-name> --tail=100
   ```

**常见原因**：
- NVIDIA Container Toolkit 未正确安装或配置
- NVIDIA Device Plugin 未运行
- Docker/containerd 配置未更新

### CPU 节点上也在尝试初始化 GPU

**检查清单**：

1. 确认使用了新版本镜像（0.1.2+）
   ```bash
   kubectl get pods -n system-info-exporter -o jsonpath='{.items[*].spec.containers[*].image}'
   ```

2. 如果版本不对，更新 DaemonSet
   ```bash
   kubectl apply -f daemonset.yaml
   kubectl rollout restart daemonset/system-info-exporter -n system-info-exporter
   ```

### Pod 无法启动

```bash
# 查看 Pod 事件
kubectl describe pod -n system-info-exporter <pod-name>

# 查看命名空间事件
kubectl get events -n system-info-exporter --sort-by='.lastTimestamp'

# 查看容器日志
kubectl logs -n system-info-exporter <pod-name>
```

## 📊 Prometheus 集成

### 部署 ServiceMonitor

如果使用 Prometheus Operator：

```bash
kubectl apply -f servicemonitor.yaml
```

### Prometheus 查询示例

```promql
# GPU 数量（按节点）
hw_gpu_count

# GPU 节点的 GPU 数量（大于 0）
hw_gpu_count > 0

# CPU 使用率
hw_cpu_usage_percent{node=~".*"}

# 内存使用率
hw_memory_usage_percent{node=~".*"}

# GPU 温度（仅 GPU 节点）
hw_gpu_temperature_celsius

# GPU 内存使用率
100 * hw_gpu_memory_used_bytes / hw_gpu_memory_total_bytes
```

### 告警规则示例

```yaml
groups:
  - name: system-info-exporter
    rules:
      # Exporter 不可用
      - alert: SystemInfoExporterDown
        expr: up{job="system-info-exporter"} == 0
        for: 5m
        annotations:
          summary: "System info exporter is down on {{ $labels.node }}"
      
      # GPU 节点但无 GPU 指标
      - alert: GPUMetricsMissing
        expr: |
          (
            count by (node) (kube_node_labels{label_nvidia_com_gpu_present="true"})
            unless
            count by (node) (hw_gpu_count > 0)
          )
        for: 10m
        annotations:
          summary: "GPU metrics missing on GPU node {{ $labels.node }}"
      
      # GPU 温度过高
      - alert: GPUHighTemperature
        expr: hw_gpu_temperature_celsius > 85
        for: 10m
        annotations:
          summary: "GPU {{ $labels.gpu_index }} temperature is {{ $value }}°C on {{ $labels.node }}"
      
      # GPU 内存使用率过高
      - alert: GPUMemoryUsageHigh
        expr: |
          (hw_gpu_memory_used_bytes / hw_gpu_memory_total_bytes) * 100 > 90
        for: 15m
        annotations:
          summary: "GPU {{ $labels.gpu_index }} memory usage > 90% on {{ $labels.node }}"
```

## 🔐 安全说明

- Pod 以非 root 用户运行（UID 1000）
- 使用只读根文件系统
- 禁用权限提升
- 移除所有 Linux capabilities
- 只挂载必要的宿主机文件（只读）

## 📝 架构优势

### 简化部署
- ✅ 单一 DaemonSet 配置
- ✅ 无需手动标记节点
- ✅ 无需额外脚本
- ✅ 自动适配 GPU 和 CPU 节点

### 智能检测
- ✅ 代码层面的 GPU 硬件检测
- ✅ 依赖 NVIDIA Container Toolkit 自动注入
- ✅ 无警告日志
- ✅ 清晰的日志输出

### 运维友好
- ✅ 统一管理
- ✅ 易于更新
- ✅ 易于监控
- ✅ 易于故障排查

## 📚 参考资料

- [NVIDIA GPU Operator 文档](https://docs.nvidia.com/datacenter/cloud-native/gpu-operator/overview.html)
- [NVIDIA Container Toolkit](https://github.com/NVIDIA/nvidia-docker)
- [Kubernetes DaemonSet](https://kubernetes.io/docs/concepts/workloads/controllers/daemonset/)

## 📄 版本兼容性

- Kubernetes: 1.20+
- NVIDIA GPU Operator: 1.9+（可选，仅 GPU 节点需要）
- NVIDIA Device Plugin: 0.12+（可选，仅 GPU 节点需要）
- Docker/containerd with NVIDIA Container Toolkit（可选，仅 GPU 节点需要）

## 🆘 获取帮助

如果遇到问题：

1. 查看 Pod 日志获取详细错误信息
2. 检查节点上的 NVIDIA Container Toolkit 配置
3. 参考故障排查部分
4. 提交 Issue 并附上详细日志

## 📄 许可证

MIT
