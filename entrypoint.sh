#!/bin/sh
# Entrypoint wrapper script for system-info-exporter
# GPU metrics are collected via nvidia-smi command (no .so library loading needed)

# Execute the main application
exec /app/system-info-exporter "$@"
