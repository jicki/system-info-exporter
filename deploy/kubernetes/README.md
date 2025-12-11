# Kubernetes Deployment

## Prerequisites

- Kubernetes cluster (v1.25+)
- kubectl configured
- NVIDIA GPU Operator or NVIDIA Device Plugin installed
- NVIDIA Container Toolkit configured

## Deploy

```bash
# Create namespace and resources
kubectl apply -f deploy/kubernetes/namespace.yaml
kubectl apply -f deploy/kubernetes/configmap.yaml
kubectl apply -f deploy/kubernetes/daemonset.yaml
kubectl apply -f deploy/kubernetes/service.yaml

# Or apply all at once
kubectl apply -f deploy/kubernetes/
```

## Verify

```bash
kubectl -n system-info-exporter get pods -o wide
kubectl -n system-info-exporter get svc
```

## Access

```bash
# Port forward for local access
kubectl -n system-info-exporter port-forward svc/system-info-exporter 8080:80

# Test endpoints
curl http://localhost:8080/health
curl http://localhost:8080/metrics
curl http://localhost:8080/node
```

## GPU Support

This exporter uses NVML (NVIDIA Management Library) to collect GPU metrics.

### Requirements

1. NVIDIA drivers installed on nodes
2. NVIDIA Container Toolkit (nvidia-docker2)
3. One of the following:
   - NVIDIA GPU Operator
   - NVIDIA Device Plugin for Kubernetes

### RuntimeClass

The DaemonSet uses `runtimeClassName: nvidia`. If your cluster uses a different
runtime class name, update the `daemonset.yaml` accordingly.

If using the default containerd with NVIDIA runtime configured, you may need to
remove or modify the `runtimeClassName` field.

### Metrics Available

| Metric | Description |
|--------|-------------|
| `hw_gpu_count` | Total number of GPUs |
| `hw_gpu_type_count` | GPU count by model |
| `hw_gpu_memory_total_bytes` | GPU memory total |
| `hw_gpu_memory_used_bytes` | GPU memory used |
| `hw_gpu_memory_free_bytes` | GPU memory free |
| `hw_gpu_utilization_percent` | GPU utilization |
| `hw_gpu_temperature_celsius` | GPU temperature |
| `hw_gpu_power_draw_watts` | GPU power draw |
| `hw_gpu_power_limit_watts` | GPU power limit |
