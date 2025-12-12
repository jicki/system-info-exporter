# Multi-Architecture Build Guide

## Problem

The "exec format error" occurs when there's an architecture mismatch between the Docker image and the target platform:

- **Local build machine**: arm64 (Apple Silicon Mac)
- **Kubernetes nodes**: amd64 (x86_64)

When you build a Docker image on an arm64 machine without specifying the target platform, it creates an arm64 binary that cannot run on amd64 nodes.

## Solution

Use Docker Buildx to create multi-architecture images that work on both amd64 and arm64 platforms.

### Prerequisites

Ensure Docker Buildx is installed and configured:

```bash
# Check if buildx is available
docker buildx version

# Create and use a new builder instance (one-time setup)
docker buildx create --name multiarch --use
docker buildx inspect --bootstrap
```

### Build Commands

#### Option 1: Build for amd64 only (Quick Fix)

For immediate deployment to x86_64 Kubernetes nodes:

```bash
make docker-build-amd64
```

This builds and pushes an image specifically for amd64 architecture.

#### Option 2: Build multi-architecture image (Recommended)

For maximum compatibility across different platforms:

```bash
make docker-build
```

This builds and pushes images for both amd64 and arm64 architectures. The Docker registry will automatically serve the correct architecture based on the node.

#### Option 3: Local build only (No push)

For testing locally without pushing to registry:

```bash
make docker-build-local
```

### Manual Build

If you need more control, use Docker buildx directly:

```bash
# Build for amd64 only
docker buildx build \
  --platform linux/amd64 \
  -t reg.deeproute.ai/deeproute-public/zzh/system-info-exporter:0.1.0 \
  --push \
  .

# Build for multiple architectures
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t reg.deeproute.ai/deeproute-public/zzh/system-info-exporter:0.1.0 \
  --push \
  .
```

### Verification

After building, verify the image architectures:

```bash
# Check available architectures in registry
docker buildx imagetools inspect \
  reg.deeproute.ai/deeproute-public/zzh/system-info-exporter:0.1.0
```

You should see output showing available platforms:

```
Name:      reg.deeproute.ai/deeproute-public/zzh/system-info-exporter:0.1.0
MediaType: application/vnd.docker.distribution.manifest.list.v2+json
Digest:    sha256:...

Manifests:
  Name:      reg.deeproute.ai/deeproute-public/zzh/system-info-exporter:0.1.0@sha256:...
  MediaType: application/vnd.docker.distribution.manifest.v2+json
  Platform:  linux/amd64

  Name:      reg.deeproute.ai/deeproute-public/zzh/system-info-exporter:0.1.0@sha256:...
  MediaType: application/vnd.docker.distribution.manifest.v2+json
  Platform:  linux/arm64
```

### Deployment

After building the correct architecture:

```bash
# Update deployment
kubectl rollout restart daemonset/system-info-exporter -n system-info-exporter

# Check logs
kubectl logs -n system-info-exporter -l app.kubernetes.io/name=system-info-exporter
```

### Common Issues

#### Error: "exec format error"

**Cause**: Binary architecture doesn't match node architecture.

**Solution**: Rebuild with correct architecture using `make docker-build-amd64`.

#### Error: "multiple platforms feature is currently not supported"

**Cause**: Using standard `docker build` instead of `docker buildx build`.

**Solution**: Use `make docker-build` which uses buildx.

#### Buildx not available

**Cause**: Docker version too old.

**Solution**: Update Docker Desktop or Docker Engine to version 19.03+.

### Best Practices

1. **Always use multi-arch builds** for production deployments to support heterogeneous clusters
2. **Verify architecture** before deploying using `docker buildx imagetools inspect`
3. **Use image digests** in production for immutable deployments
4. **Cache builder instances** to speed up builds

### Performance Notes

Multi-architecture builds take longer because they compile for multiple targets. For development:

- Use `make docker-build-local` for quick local testing
- Use `make docker-build-amd64` when deploying to x86_64 clusters
- Use `make docker-build` for production multi-arch images

### CI/CD Integration

For automated builds in CI/CD pipelines:

```yaml
# Example GitHub Actions
- name: Set up Docker Buildx
  uses: docker/setup-buildx-action@v2

- name: Build and push
  run: make docker-build
```

### References

- [Docker Buildx documentation](https://docs.docker.com/buildx/working-with-buildx/)
- [Multi-platform images](https://docs.docker.com/build/building/multi-platform/)
