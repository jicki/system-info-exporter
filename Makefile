.PHONY: all build run test clean docker-build docker-push docker-run help

# Project variables
APP_NAME := system-info-exporter
VERSION := $(shell cat VERSION)
DOCKER_REGISTRY ?= docker.io
DOCKER_REPO ?= $(DOCKER_REGISTRY)/$(APP_NAME)
DOCKER_TAG ?= $(VERSION)

# Build variables
CARGO := cargo
RUSTFLAGS := -C target-feature=-crt-static

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

## docker-build: Build Docker image
docker-build:
	docker build -t $(DOCKER_REPO):$(DOCKER_TAG) .
	docker tag $(DOCKER_REPO):$(DOCKER_TAG) $(DOCKER_REPO):latest

## docker-push: Push Docker image to registry
docker-push:
	docker push $(DOCKER_REPO):$(DOCKER_TAG)
	docker push $(DOCKER_REPO):latest

## docker-run: Run Docker container locally
docker-run:
	docker run --rm -p 8080:8080 $(DOCKER_REPO):$(DOCKER_TAG)

## version: Show version
version:
	@echo $(VERSION)
