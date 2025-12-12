#!/bin/bash
# Entrypoint wrapper script for system-info-exporter
# Sets LD_LIBRARY_PATH dynamically based on NVML library location

# Search paths for NVML library
NVML_PATHS=(
    "/host/nvidia-libs"
    "/usr/lib/x86_64-linux-gnu"
    "/usr/lib"
)

# Find and set NVML library path
for dir in "${NVML_PATHS[@]}"; do
    if [ -f "$dir/libnvidia-ml.so" ] || [ -f "$dir/libnvidia-ml.so.1" ]; then
        export LD_LIBRARY_PATH="$dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        echo "Set LD_LIBRARY_PATH=$LD_LIBRARY_PATH"
        break
    fi
done

# Execute the main application
exec /app/system-info-exporter "$@"
