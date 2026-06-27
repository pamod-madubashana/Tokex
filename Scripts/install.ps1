#Requires -Version 5.1
<#
.SYNOPSIS
    Installs Cotrex on Windows.
.DESCRIPTION
    Downloads the latest Cotrex release and installs it to ~/.local/bin.
.PARAMETER InstallDir
    Installation directory. Default: ~/.local/bin
#>
param(
    [string]$InstallDir = "$env:USERPROFILE\.local\bin"
)

$ErrorActionPreference = "Stop"
$Repo = "pamod-madubashana/Cotrex"

Write-Host "Installing Cotrex..." -ForegroundColor Cyan

# Detect architecture
$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { Write-Host "Unsupported: 32-bit Windows"; exit 1 }
$Filename = "cotrex-VERSION-windows-$Arch.zip" -replace "VERSION", ""

# Get latest release
Write-Host "Fetching latest release..."
$Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
$Tag = $Release.tag_name
$Version = $Tag -replace "^v", ""
$Filename = "cotrex-$Version-windows-$Arch.zip"
$Url = "https://github.com/$Repo/releases/download/$Tag/$Filename"

Write-Host "Downloading $Filename..."
$TmpDir = Join-Path $env:TEMP "cotrex-install-$(Get-Random)"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    $ZipPath = Join-Path $TmpDir $Filename
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing

    Write-Host "Extracting..."
    Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

    # Install
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item (Join-Path $TmpDir "cotrex.exe") (Join-Path $InstallDir "cotrex.exe") -Force

    Write-Host ""
    Write-Host "Installed: $InstallDir\cotrex.exe" -ForegroundColor Green

    # PATH check
    $PathDirs = $env:PATH -split ";"
    if ($PathDirs -notcontains $InstallDir) {
        Write-Host ""
        Write-Host "Add to your PATH:" -ForegroundColor Yellow
        Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `"`$env:PATH;$InstallDir`", 'User')"
    }

    Write-Host ""
    & "$InstallDir\cotrex.exe" --version
} finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
