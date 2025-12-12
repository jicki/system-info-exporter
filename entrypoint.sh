#!/bin/sh
# Entrypoint wrapper script for system-info-exporter
# Sets LD_LIBRARY_PATH dynamically based on NVML library location
#
# With musl static compilation, setting LD_LIBRARY_PATH to /host/nvidia-libs
# is SAFE because:
# - The binary has no glibc dependency (statically linked with musl)
# - LD_LIBRARY_PATH only affects dlopen() for loading libnvidia-ml.so
# - The host's libc.so.6 in /host/nvidia-libs will NOT be loaded

# Search paths for NVML library
NVML_PATHS="/host/nvidia-libs /usr/lib/x86_64-linux-gnu /usr/lib"

# Find and set NVML library path
for dir in $NVML_PATHS; do
    if [ -f "$dir/libnvidia-ml.so" ] || [ -f "$dir/libnvidia-ml.so.1" ]; then
        if [ -n "$LD_LIBRARY_PATH" ]; then
            export LD_LIBRARY_PATH="$dir:$LD_LIBRARY_PATH"
        else
            export LD_LIBRARY_PATH="$dir"
        fi
        echo "Set LD_LIBRARY_PATH=$LD_LIBRARY_PATH"
        break
    fi
done

# Execute the main application
exec /app/system-info-exporter "$@"
