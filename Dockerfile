# Build stage - use Rust with musl target for static compilation
FROM reg.deeproute.ai/deeproute-public/zzh/rust:1.92-alpine AS builder

RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.tuna.tsinghua.edu.cn/g' /etc/apk/repositories

# Install build dependencies for musl static compilation
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static

# Set environment for static linking
ENV OPENSSL_STATIC=1
ENV OPENSSL_DIR=/usr

WORKDIR /app

# Copy manifest
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to cache dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl && \
    rm -rf src target/x86_64-unknown-linux-musl/release/system-info-exporter*

# Copy actual source code
COPY src ./src
COPY config ./config

# Build the application with musl (fully static binary)
RUN touch src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl

# Verify it's statically linked
RUN file target/x86_64-unknown-linux-musl/release/system-info-exporter && \
    ldd target/x86_64-unknown-linux-musl/release/system-info-exporter 2>&1 || true

# Runtime stage - minimal base image (can even use scratch)
FROM reg.deeproute.ai/deeproute-public/zzh/alpine:3.23

RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.tuna.tsinghua.edu.cn/g' /etc/apk/repositories

# Install minimal runtime dependencies
RUN apk add --no-cache ca-certificates bash

# Create non-root user
RUN addgroup -g 1000 appgroup && \
    adduser -u 1000 -G appgroup -s /bin/bash -D appuser

WORKDIR /app

# Copy binary and config from builder
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/system-info-exporter /app/
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
