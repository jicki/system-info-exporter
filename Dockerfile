# Build stage - use Rust official image with CUDA headers
FROM reg.deeproute.ai/deeproute-public/zzh/rust:1.92-slim-bookworm AS builder

# Install CUDA headers and build dependencies
# Only install what's needed for nvml-wrapper compilation
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Download and install minimal CUDA toolkit (only NVML headers)
# Only install on x86_64 architecture, skip on ARM64
RUN ARCH=$(dpkg --print-architecture) && \
    if [ "$ARCH" = "amd64" ]; then \
        mkdir -p /usr/local/cuda/include && \
        wget -q https://developer.download.nvidia.com/compute/cuda/repos/debian11/x86_64/cuda-keyring_1.1-1_all.deb && \
        dpkg -i cuda-keyring_1.1-1_all.deb && \
        apt-get update && \
        apt-get install -y cuda-nvml-dev-12-3 --no-install-recommends && \
        rm -rf /var/lib/apt/lists/* && \
        rm cuda-keyring_1.1-1_all.deb; \
    else \
        echo "Skipping CUDA installation on non-x86_64 architecture: $ARCH"; \
    fi

WORKDIR /app

# Copy manifest
COPY Cargo.toml ./

# Create dummy src to cache dependencies
# This layer will be cached unless dependencies change
# Note: Cargo.lock will be generated if it doesn't exist
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/system-info-exporter*

# Copy actual source code
COPY src ./src
COPY config ./config

# Build the application
# Touch main.rs to force rebuild of only our code
RUN touch src/main.rs && \
    cargo build --release

# Runtime stage - minimal base image
FROM reg.deeproute.ai/deeproute-public/zzh/debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -g 1000 appgroup && \
    useradd -u 1000 -g appgroup -s /bin/bash -m appuser

WORKDIR /app

# Copy binary and config from builder
COPY --from=builder /app/target/release/system-info-exporter /app/
COPY --from=builder /app/config /app/config

# Set ownership
RUN chown -R appuser:appgroup /app

USER appuser

EXPOSE 8080

ENV RUST_LOG=info
ENV NVIDIA_VISIBLE_DEVICES=all
ENV NVIDIA_DRIVER_CAPABILITIES=utility

ENTRYPOINT ["/app/system-info-exporter"]
