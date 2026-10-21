param(
    [string]$PackageName = $env:MSIX_PACKAGE_NAME,
    [string]$Publisher = $env:MSIX_PUBLISHER,
    [string]$PublisherDisplayName = $env:MSIX_PUBLISHER_DISPLAY_NAME,
    [string]$Version = "1.0.4.0",
    [string]$BinaryPath = "target\x86_64-pc-windows-msvc\release\nocturne-daemon.exe",
    [string]$OutputFile = "nocturne-windows-x64.msix"
)

$ErrorActionPreference = "Stop"

if (-not $PackageName) {
    Write-Warning "MSIX_PACKAGE_NAME not provided, using fallback 'local.nocturne.mk4.daemon'"
    $PackageName = "local.nocturne.mk4.daemon"
}

if (-not $Publisher) {
    Write-Warning "MSIX_PUBLISHER not provided, using fallback 'CN=NocturneDevelopment'"
    $Publisher = "CN=NocturneDevelopment"
}

if (-not $PublisherDisplayName) {
    Write-Warning "MSIX_PUBLISHER_DISPLAY_NAME not provided, using fallback 'Nocturne Team'"
    $PublisherDisplayName = "Nocturne Team"
}

Write-Host "========================================"
Write-Host "Building MSIX Package for Nocturne"
Write-Host "Package Name:          $PackageName"
Write-Host "Publisher:             $Publisher"
Write-Host "Publisher Display Name:$PublisherDisplayName"
Write-Host "Version:               $Version"
Write-Host "Binary Source:         $BinaryPath"
Write-Host "Output File:           $OutputFile"
Write-Host "========================================"

if (-not (Test-Path $BinaryPath)) {
    Write-Error "Binary not found at $BinaryPath. Please ensure cargo/tauri build completed."
    exit 1
}

# 1. Setup Staging Directory
$stageDir = "target\msix_stage"
if (Test-Path $stageDir) {
    Remove-Item -Recurse -Force $stageDir
}
New-Item -ItemType Directory -Path $stageDir -Force | Out-Null
New-Item -ItemType Directory -Path "$stageDir\Assets" -Force | Out-Null

# 2. Copy Executable and Assets
Copy-Item -Path $BinaryPath -Destination "$stageDir\nocturne-daemon.exe" -Force
Copy-Item -Path "src-tauri\msix\Assets\*" -Destination "$stageDir\Assets" -Force

# 3. Process AppxManifest.xml
$manifestTemplate = Get-Content "src-tauri\msix\AppxManifest.xml.template" -Raw
$manifestContent = $manifestTemplate `
    -replace "\{\{PACKAGE_NAME\}\}", $PackageName `
    -replace "\{\{PUBLISHER\}\}", $Publisher `
    -replace "\{\{PUBLISHER_DISPLAY_NAME\}\}", $PublisherDisplayName `
    -replace "\{\{VERSION\}\}", $Version

Set-Content -Path "$stageDir\AppxManifest.xml" -Value $manifestContent -Encoding UTF8

# 4. Locate MakeAppx.exe from Windows SDK
$kitsRoot = "C:\Program Files (x86)\Windows Kits\10\bin"
if (-not (Test-Path $kitsRoot)) {
    Write-Error "Windows Kits 10 path not found at $kitsRoot"
    exit 1
}

$sdkDirs = Get-ChildItem -Path $kitsRoot | Where-Object { $_.PSIsContainer -and ($_.Name -match '^\d+\.') }
if (-not $sdkDirs) {
    Write-Error "No Windows 10/11 SDK versions found in $kitsRoot"
    exit 1
}

$latestSdk = ($sdkDirs | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1).FullName
$makeAppx = Join-Path $latestSdk "x64\makeappx.exe"

if (-not (Test-Path $makeAppx)) {
    Write-Error "makeappx.exe not found at $makeAppx"
    exit 1
}

Write-Host "Using MakeAppx tool: $makeAppx"

# 5. Pack MSIX
Write-Host "Executing MakeAppx pack..."
& $makeAppx pack /d $stageDir /p $OutputFile /o

if ($LASTEXITCODE -ne 0) {
    Write-Error "MakeAppx failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

if (-not (Test-Path $OutputFile)) {
    Write-Error "Expected MSIX output not found at $OutputFile"
    exit 1
}

$item = Get-Item $OutputFile
$sizeMb = [math]::Round($item.Length / 1MB, 2)
Write-Host "Successfully generated MSIX: $OutputFile ($sizeMb MB)"
