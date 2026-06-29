# One-click PowerShell script to clean previous builds, check/update dependencies, and run the monitor.

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "   Terminal System Monitor - Launching (ps1)      " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

Write-Host "[1/3] Cleaning previous build targets..." -ForegroundColor Yellow
cargo clean

Write-Host "[2/3] Checking and updating dependencies if needed..." -ForegroundColor Yellow
cargo check

Write-Host "[3/3] Starting system monitor..." -ForegroundColor Green
cargo run
