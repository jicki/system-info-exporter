# Build stage - use Rust Debian for glibc compatibility with host nvidia libraries
FROM reg.deeproute.ai/deeproute-public/zzh/rust:1.92-silm-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifest
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to cache dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/system-info-exporter*

# Copy actual source code
COPY src ./src
COPY config ./config

# Build the application
RUN touch src/main.rs && \
    cargo build --release

# Runtime stage - Debian slim with glibc (compatible with host nvidia libraries)
FROM reg.deeproute.ai/deeproute-public/zzh/debian:bookworm-slim

# Install minimal runtime dependencies
# coreutils provides 'timeout' command for nvidia-smi timeout handling
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    coreutils \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -g 1000 appgroup && \
    useradd -u 1000 -g appgroup -s /bin/sh -M appuser

WORKDIR /app

# Copy binary and config from builder
COPY --from=builder /app/target/release/system-info-exporter /app/
COPY --from=builder /app/config /app/config
COPY entrypoint.sh /app/

# Set ownership and permissions
RUN chown -R appuser:appgroup /app && \
    chmod +x /app/entrypoint.sh

USER appuser

EXPOSE 8080

ENV RUST_LOG=info
ENV NVIDIA_VISIBLE_DEVICES=all
ENV NVIDIA_DRIVER_CAPABILITIES=utility

ENTRYPOINT ["/app/entrypoint.sh"]
