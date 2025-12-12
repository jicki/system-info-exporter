.PHONY: all build run test clean docker-build docker-run help

# Project variables
APP_NAME := system-info-exporter
VERSION := $(shell cat VERSION)
DOCKER_REGISTRY ?= reg.deeproute.ai/deeproute-public/zzh
DOCKER_REPO ?= $(DOCKER_REGISTRY)/$(APP_NAME)
DOCKER_TAG ?= $(VERSION)

# Build variables
CARGO := cargo
RUSTFLAGS := -C target-feature=-crt-static
DOCKER_BUILDKIT ?= 1

# Default target
all: build

## help: Show this help message
help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^## ' Makefile | sed 's/## /  /'

## build: Build the application
build:
	$(CARGO) build --release

## run: Run the application locally
run:
	$(CARGO) run

## test: Run tests
test:
	$(CARGO) test

## fmt: Format code
fmt:
	$(CARGO) fmt

## lint: Run clippy linter
lint:
	$(CARGO) clippy -- -D warnings

## check: Run format check and linter
check: fmt lint

## clean: Clean build artifacts
clean:
	$(CARGO) clean
	rm -rf target/

## docker-build: Build and push multi-arch Docker image (amd64 and arm64)
docker-build:
	@echo "Building multi-architecture Docker image with BuildKit..."
	docker buildx build \
		--platform linux/amd64,linux/arm64 \
		-t $(DOCKER_REPO):$(DOCKER_TAG) \
		-t $(DOCKER_REPO):latest \
		--push \
		.
	@echo "Multi-arch build and push complete!"

## docker-build-amd64: Build and push Docker image for amd64 only (for x86_64 K8s nodes)
docker-build-amd64:
	@echo "Building Docker image for amd64 architecture..."
	docker buildx build \
		--platform linux/amd64 \
		-t $(DOCKER_REPO):$(DOCKER_TAG) \
		-t $(DOCKER_REPO):latest \
		--push \
		.
	@echo "AMD64 build and push complete!"

## docker-build-local: Build Docker image for local architecture only (no push)
docker-build-local:
	@echo "Building Docker image for local architecture..."
	DOCKER_BUILDKIT=$(DOCKER_BUILDKIT) docker build \
		-t $(DOCKER_REPO):$(DOCKER_TAG) \
		-t $(DOCKER_REPO):latest \
		.
	@echo "Local build complete!"

## docker-run: Run Docker container locally
docker-run:
	docker run --rm -p 8080:8080 --gpus all $(DOCKER_REPO):$(DOCKER_TAG)

## version: Show version
version:
	@echo $(VERSION)
