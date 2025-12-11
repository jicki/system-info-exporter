# Build stage - using CUDA image for NVML support
FROM nvidia/cuda:13.0.2-devel-ubuntu22.04 AS builder

# Install Rust and build dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to cache dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build the application
RUN touch src/main.rs && \
    cargo build --release

# Runtime stage - using CUDA runtime image (smaller than devel)
FROM nvidia/cuda:13.0.2-base-ubuntu22.04

RUN apt-get update && apt-get install -y \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -g 1000 appgroup && \
    useradd -u 1000 -g appgroup -s /bin/bash -m appuser

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/system-info-exporter /app/
COPY config /app/config

# Set ownership
RUN chown -R appuser:appgroup /app

USER appuser

EXPOSE 8080

ENV RUST_LOG=info
ENV NVIDIA_VISIBLE_DEVICES=all
ENV NVIDIA_DRIVER_CAPABILITIES=utility

ENTRYPOINT ["/app/system-info-exporter"]
