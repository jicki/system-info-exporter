.PHONY: all build run test clean docker-build docker-run help

# Project variables
APP_NAME := system-info-exporter
VERSION := $(shell cat VERSION)
DOCKER_REGISTRY ?= reg.deeproute.ai/deeproute-public/zzh
DOCKER_REPO ?= $(DOCKER_REGISTRY)/$(APP_NAME)
DOCKER_TAG ?= $(VERSION)

# Build variables
CARGO := cargo
DOCKER_BUILDKIT ?= 1

# Default target
all: build

## help: Show this help message
help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@grep -E '^## ' Makefile | sed 's/## /  /'

## build: Build the application (native)
build:
	$(CARGO) build --release

## build-musl: Build the application with musl (requires musl-tools)
build-musl:
	$(CARGO) build --release --target x86_64-unknown-linux-musl

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

## docker-build: Build and push Docker image for amd64 (uses nvidia-smi, no .so dependency)
docker-build:
	@echo "Building Docker image..."
	docker buildx build \
		--platform linux/amd64 \
		-t $(DOCKER_REPO):$(DOCKER_TAG) \
		-t $(DOCKER_REPO):latest \
		--push \
		.
	@echo "Build and push complete!"

## docker-build-local: Build Docker image locally (no push)
docker-build-local:
	@echo "Building Docker image locally..."
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
