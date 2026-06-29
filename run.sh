#!/bin/bash
# One-click shell script to clean previous builds, check/update dependencies, and run the monitor.

echo "=================================================="
echo "   Terminal System Monitor - Launching (sh)       "
echo "=================================================="

echo "[1/3] Cleaning previous build targets..."
cargo clean

echo "[2/3] Checking and updating dependencies if needed..."
cargo check

echo "[3/3] Starting system monitor..."
cargo run
