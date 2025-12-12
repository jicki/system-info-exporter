# 更新日志

## v0.1.2 (2025-12-12)

### 🎯 问题修复

修复了混合集群（GPU + CPU 节点）部署时的问题：

1. **GPU 节点**：NVML 初始化失败，无法采集 GPU 指标
2. **CPU 节点**：产生大量 NVML 初始化警告日志

### ✨ 新特性

#### 智能 GPU 硬件检测

在代码层面添加了 GPU 硬件检测逻辑（`src/metrics.rs`）：

```rust
fn has_nvidia_gpu() -> bool {
    std::path::Path::new("/dev/nvidiactl").exists()
        || std::path::Path::new("/dev/nvidia0").exists()
        || std::path::Path::new("/proc/driver/nvidia/version").exists()
}
```

**工作原理**：
- 在尝试初始化 NVML 之前，先检查 NVIDIA 设备文件是否存在
- **GPU 节点**：检测到设备文件 → 正常初始化 NVML → 采集 GPU 指标
- **CPU 节点**：未检测到设备文件 → 跳过 NVML 初始化 → 只采集 CPU/内存指标

### 🏗️ 架构优化

#### 简化的统一部署

更新了 Kubernetes DaemonSet 配置（`deploy/kubernetes/daemonset.yaml`）：

**主要改动**：
- ❌ 移除 `runtimeClassName: nvidia`（不再强制要求）
- ❌ 移除 `nodeSelector`（支持所有节点）
- ✅ 保留 `NVIDIA_VISIBLE_DEVICES` 环境变量（让 Container Toolkit 自动处理）

**效果**：
- GPU 节点：NVIDIA Container Toolkit 自动注入 GPU 支持
- CPU 节点：环境变量被忽略，应用通过硬件检测自动跳过 GPU

### 📊 部署架构

```
统一 DaemonSet
├─ 部署到所有节点（GPU + CPU）
├─ GPU 节点：
│  ├─ NVIDIA Container Toolkit 自动注入 GPU 支持 ✅
│  ├─ has_nvidia_gpu() 返回 true ✅
│  └─ 采集 CPU + 内存 + GPU 指标 ✅
└─ CPU 节点：
   ├─ 无 NVIDIA Container Toolkit ✅
   ├─ has_nvidia_gpu() 返回 false ✅
   └─ 只采集 CPU + 内存指标 ✅
```

### 📈 预期效果

#### GPU 节点日志
```json
{"level":"INFO","message":"Starting system-info-exporter"}
{"level":"INFO","message":"Collecting metrics..."}
// GPU 指标正常采集
```

#### CPU 节点日志
```json
{"level":"INFO","message":"Starting system-info-exporter"}
{"level":"INFO","message":"No NVIDIA GPU hardware detected, skipping GPU metrics collection"}
// 只采集 CPU 和内存指标
```

#### 不再有的错误
- ❌ `Failed to initialize NVML: libnvidia-ml.so: cannot open shared object file`
- ❌ 大量 WARN 级别的 NVML 初始化失败日志

### 🚀 升级指南

#### 1. 构建新镜像

```bash
make docker-build-amd64
```

#### 2. 更新部署

```bash
# 更新 DaemonSet
kubectl apply -f deploy/kubernetes/daemonset.yaml

# 或直接更新镜像
kubectl set image daemonset/system-info-exporter \
  system-info-exporter=reg.deeproute.ai/deeproute-public/zzh/system-info-exporter:0.1.2 \
  -n system-info-exporter
```

#### 3. 验证部署

```bash
# 查看 Pod 状态
kubectl get pods -n system-info-exporter -o wide

# 查看日志
kubectl logs -n system-info-exporter <pod-name> --tail=50
```

### 📁 修改的文件

```
src/metrics.rs                       # 添加 GPU 硬件检测逻辑
deploy/kubernetes/daemonset.yaml     # 简化配置，支持混合节点
deploy/kubernetes/README.md          # 更新部署文档
VERSION                              # 更新版本号到 0.1.2
```

### 🔧 技术细节

#### GPU 检测逻辑

通过检测以下设备文件判断是否为 GPU 节点：
- `/dev/nvidiactl` - NVIDIA 驱动控制设备
- `/dev/nvidia0` - 第一个 GPU 设备
- `/proc/driver/nvidia/version` - NVIDIA 驱动版本信息

只要任意一个文件存在，就认为是 GPU 节点。

#### NVIDIA Container Toolkit 集成

- GPU 节点上配置了 NVIDIA Container Toolkit
- 通过 `NVIDIA_VISIBLE_DEVICES=all` 环境变量触发自动注入
- Toolkit 自动挂载 GPU 设备和 NVIDIA 库文件
- 应用可以正常访问 `libnvidia-ml.so` 和 GPU 设备

### 🎉 优势

- ✅ **简单**：单一 DaemonSet 配置，无需手动区分节点
- ✅ **智能**：自动检测 GPU 硬件，无需额外配置
- ✅ **清晰**：无警告日志，日志输出更清晰
- ✅ **可靠**：依赖标准的 NVIDIA Container Toolkit
- ✅ **易维护**：统一管理，易于更新和监控

### 📚 参考文档

- [Kubernetes 部署指南](deploy/kubernetes/README.md)
- [NVIDIA Container Toolkit](https://github.com/NVIDIA/nvidia-docker)

---

## v0.1.1 及更早版本

参见 Git 提交历史。
