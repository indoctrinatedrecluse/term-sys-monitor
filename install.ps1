# install.ps1 - Installer for Windows

$ErrorActionPreference = "Stop"

$Repo = "indoctrinatedrecluse/term-sys-monitor"
$BinaryName = "term-sys-monitor-windows.exe"
$InstallFolder = Join-Path $env:USERPROFILE ".sysmon\bin"

# Ensure install directory exists
if (-not (Test-Path $InstallFolder)) {
    New-Item -ItemType Directory -Force -Path $InstallFolder | Out-Null
}

Write-Host "Fetching latest release version from GitHub..." -ForegroundColor Cyan
try {
    $ReleaseInfo = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $LatestTag = $ReleaseInfo.tag_name
} catch {
    Write-Error "Could not retrieve latest release version. Make sure there is a release available on GitHub."
}

$DownloadUrl = "https://github.com/$Repo/releases/download/$LatestTag/$BinaryName"
$DestPath = Join-Path $InstallFolder "term-sys-monitor.exe"

Write-Host "Downloading term-sys-monitor ($LatestTag)..." -ForegroundColor Yellow
Invoke-WebRequest -Uri $DownloadUrl -OutFile $DestPath -UseBasicParsing

# Update User PATH environment variable permanently in registry
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallFolder*") {
    Write-Host "Adding $InstallFolder to User PATH..." -ForegroundColor Cyan
    # Append path separator and install folder path
    $NewPath = $UserPath.TrimEnd(';') + ";" + $InstallFolder
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    # Update current session path
    $env:Path += ";" + $InstallFolder
}

Write-Host "==================================================" -ForegroundColor Green
Write-Host "   term-sys-monitor successfully installed!      " -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
Write-Host "Location: $DestPath" -ForegroundColor Gray
Write-Host "Simply restart your terminal and run: term-sys-monitor" -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
