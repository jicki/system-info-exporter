# Build stage - use Rust Alpine for musl dynamic linking (supports dlopen)
FROM reg.deeproute.ai/deeproute-public/zzh/rust:1.92-alpine AS builder

RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.tuna.tsinghua.edu.cn/g' /etc/apk/repositories

# Install build dependencies for musl dynamic compilation
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev

WORKDIR /app

# Copy manifest
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to cache dependencies
# Use native target (alpine's musl) - NOT --target x86_64-unknown-linux-musl
# This creates a dynamically linked binary that supports dlopen
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/system-info-exporter*

# Copy actual source code
COPY src ./src
COPY config ./config

# Build the application (dynamically linked to musl libc, supports dlopen)
RUN touch src/main.rs && \
    cargo build --release && \
    ldd target/release/system-info-exporter

# Runtime stage - Alpine with musl libc
FROM reg.deeproute.ai/deeproute-public/zzh/alpine:3.23

RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.tuna.tsinghua.edu.cn/g' /etc/apk/repositories

# Install minimal runtime dependencies
RUN apk add --no-cache ca-certificates

# Create non-root user
RUN addgroup -g 1000 appgroup && \
    adduser -u 1000 -G appgroup -s /bin/sh -D appuser

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
