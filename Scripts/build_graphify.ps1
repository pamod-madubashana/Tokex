# build_graphify.ps1 - Build graphify as a standalone executable
# Usage: .\Scripts\build_graphify.ps1

$ErrorActionPreference = "Stop"

Write-Host "Building graphify standalone executable..." -ForegroundColor Cyan

# Check Python
$python = Get-Command python -ErrorAction SilentlyContinue
if (-not $python) {
    Write-Error "Python not found. Please install Python 3.10+"
    exit 1
}

# Check/install PyInstaller
try {
    python -c "import PyInstaller" 2>$null
    if ($LASTEXITCODE -ne 0) { throw "not installed" }
    $ver = python -c "import PyInstaller; print(PyInstaller.__version__)"
    Write-Host "PyInstaller version: $ver" -ForegroundColor Green
} catch {
    Write-Host "Installing PyInstaller..." -ForegroundColor Yellow
    python -m pip install pyinstaller
    if ($LASTEXITCODE -ne 0) { Write-Error "Failed to install PyInstaller"; exit 1 }
}

# Check/install graphify
try {
    python -c "import graphify" 2>$null
    if ($LASTEXITCODE -ne 0) { throw "not installed" }
    Write-Host "graphify package found" -ForegroundColor Green
} catch {
    Write-Host "Installing graphify..." -ForegroundColor Yellow
    python -m pip install graphifyy
    if ($LASTEXITCODE -ne 0) { Write-Error "Failed to install graphify"; exit 1 }
}

# Clean previous builds
$distDir = Join-Path $PSScriptRoot "..\dist"
$buildDir = Join-Path $PSScriptRoot "..\build"
if (Test-Path $distDir) { Remove-Item -Path $distDir -Recurse -Force }
if (Test-Path $buildDir) { Remove-Item -Path $buildDir -Recurse -Force }

# Run PyInstaller
$specFile = Join-Path $PSScriptRoot "graphify.spec"
Write-Host "Running PyInstaller..." -ForegroundColor Cyan
python -m PyInstaller --clean --noconfirm --specpath (Split-Path $specFile) $specFile
if ($LASTEXITCODE -ne 0) {
    Write-Error "PyInstaller build failed!"
    exit 1
}

# Find built executable
$graphifyExe = Join-Path $distDir "graphify.exe"
if (-not (Test-Path $graphifyExe)) {
    Write-Error "Expected executable not found at $graphifyExe"
    exit 1
}

# Copy to target/release
$targetDir = Join-Path $PSScriptRoot "..\target\release"
if (-not (Test-Path $targetDir)) { New-Item -ItemType Directory -Path $targetDir -Force | Out-Null }
$dest = Join-Path $targetDir "graphify.exe"
Copy-Item -Path $graphifyExe -Destination $dest -Force
$size = (Get-Item $dest).Length / 1MB
Write-Host "Built graphify executable: $dest" -ForegroundColor Green
Write-Host "Size: $([math]::Round($size, 1)) MB" -ForegroundColor Green

# Clean up
if (Test-Path $distDir) { Remove-Item -Path $distDir -Recurse -Force }
if (Test-Path $buildDir) { Remove-Item -Path $buildDir -Recurse -Force }

Write-Host "Done!" -ForegroundColor Cyan
