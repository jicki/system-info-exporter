# Kubernetes Deployment

## Prerequisites

- Kubernetes cluster (v1.25+)
- kubectl configured

## Deploy

```bash
# Create namespace and resources
kubectl apply -f deploy/kubernetes/namespace.yaml
kubectl apply -f deploy/kubernetes/configmap.yaml
kubectl apply -f deploy/kubernetes/deployment.yaml
kubectl apply -f deploy/kubernetes/service.yaml

# Or apply all at once
kubectl apply -f deploy/kubernetes/
```

## Verify

```bash
kubectl -n system-info-exporter get pods
kubectl -n system-info-exporter get svc
```

## Access

```bash
# Port forward for local access
kubectl -n system-info-exporter port-forward svc/system-info-exporter 8080:80

# Test endpoints
curl http://localhost:8080/health
curl http://localhost:8080/metrics
```
