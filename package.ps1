<#
.SYNOPSIS
    Packages Twitch TTS into a self-contained, standalone distribution directory.
.DESCRIPTION
    Builds the release binary and copies twitch-tts.exe, all ONNX Runtime / DirectML / CUDA provider DLLs
    (resolving symlinks to real physical files), models, and voices into a 'dist' directory.
#>

param (
    [string]$OutputDir = "$PSScriptRoot\dist",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Packaging Twitch TTS for Standalone Release   " -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan

if (-not $SkipBuild) {
    Write-Host "`n[1/5] Building release binary..." -ForegroundColor Yellow
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Cargo release build failed with exit code $LASTEXITCODE"
    }
} else {
    Write-Host "`n[1/5] Skipping build (-SkipBuild specified)..." -ForegroundColor DarkGray
}

$targetRelease = "$PSScriptRoot\target\release"
$exePath = "$targetRelease\twitch-tts.exe"

if (-not (Test-Path $exePath)) {
    Write-Error "Could not find $exePath. Please run cargo build --release first."
}

Write-Host "`n[2/5] Creating distribution directory at '$OutputDir'..." -ForegroundColor Yellow
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

Write-Host "`n[3/5] Copying executable and resolving runtime DLLs..." -ForegroundColor Yellow
# 1. Copy EXE
Copy-Item $exePath -Destination "$OutputDir\twitch-tts.exe" -Force
Write-Host "  -> Copied twitch-tts.exe" -ForegroundColor Green

# 2. Copy DLLs (Dereferencing NTFS symlinks created by ort copy-dylibs)
$dlls = Get-ChildItem -Path $targetRelease -Filter "*.dll"
foreach ($dll in $dlls) {
    $srcPath = $dll.FullName
    if ($dll.LinkType -eq "SymbolicLink" -or $dll.Target) {
        $realPath = (Get-Item -LiteralPath $srcPath).Target
        if ($realPath -and (Test-Path $realPath)) {
            $srcPath = $realPath
        }
    }

    $destPath = Join-Path $OutputDir $dll.Name
    [System.IO.File]::Copy($srcPath, $destPath, $true)
    $sizeMb = [math]::Round(((Get-Item $destPath).Length / 1MB), 2)
    Write-Host "  -> Copied $($dll.Name) ($sizeMb MB, resolved)" -ForegroundColor Green
}

# 2b. Copy CUDA cuBLAS runtime DLLs (cublas64_*.dll, cublasLt64_*.dll)
Write-Host "`n  -> Searching for CUDA cuBLAS DLLs..." -ForegroundColor DarkCyan
$cudaSearchDirs = @()
if ($env:CUDA_PATH) {
    $cudaSearchDirs += "$env:CUDA_PATH\bin\x64"
    $cudaSearchDirs += "$env:CUDA_PATH\bin"
}
$cudaBase = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA"
if (Test-Path $cudaBase) {
    $subdirs = Get-ChildItem -Path $cudaBase -Directory
    foreach ($sub in $subdirs) {
        $cudaSearchDirs += "$($sub.FullName)\bin\x64"
        $cudaSearchDirs += "$($sub.FullName)\bin"
    }
}

$cublasFound = $false
foreach ($cDir in $cudaSearchDirs) {
    if (Test-Path $cDir) {
        $cublasFiles = Get-ChildItem -Path $cDir -Filter "cublas*.dll"
        if ($cublasFiles.Count -gt 0) {
            foreach ($cDll in $cublasFiles) {
                $dest = Join-Path $OutputDir $cDll.Name
                if (-not (Test-Path $dest) -or ((Get-Item $dest).Length -ne $cDll.Length)) {
                    Copy-Item $cDll.FullName -Destination $dest -Force
                    $cSizeMb = [math]::Round(($cDll.Length / 1MB), 2)
                    Write-Host "  -> Copied $($cDll.Name) ($cSizeMb MB from $cDir)" -ForegroundColor Green
                } else {
                    Write-Host "  -> $($cDll.Name) already up to date" -ForegroundColor DarkGray
                }
            }
            $cublasFound = $true
            break
        }
    }
}

if (-not $cublasFound) {
    Write-Warning "Could not find CUDA cuBLAS DLLs in standard locations ($cudaBase)."
}

# 3. Copy models directory
Write-Host "`n[4/5] Copying models directory..." -ForegroundColor Yellow
$modelsSrc = "$PSScriptRoot\models"
$modelsDest = "$OutputDir\models"
if (Test-Path $modelsSrc) {
    if (-not (Test-Path $modelsDest)) {
        New-Item -ItemType Directory -Path $modelsDest -Force | Out-Null
    }
    $modelFiles = Get-ChildItem -Path $modelsSrc
    foreach ($m in $modelFiles) {
        $dest = Join-Path $modelsDest $m.Name
        if (-not (Test-Path $dest) -or ((Get-Item $dest).Length -ne $m.Length)) {
            Copy-Item $m.FullName -Destination $dest -Force
            $mSizeMb = [math]::Round(($m.Length / 1MB), 1)
            Write-Host "  -> Copied model $($m.Name) ($mSizeMb MB)" -ForegroundColor Green
        } else {
            Write-Host "  -> Model $($m.Name) already up to date" -ForegroundColor DarkGray
        }
    }
} else {
    Write-Warning "No 'models' directory found at $modelsSrc!"
}

# 4. Copy voices directory
Write-Host "`n[5/5] Copying voices directory..." -ForegroundColor Yellow
$voicesSrc = "$PSScriptRoot\voices"
$voicesDest = "$OutputDir\voices"
if (Test-Path $voicesSrc) {
    if (-not (Test-Path $voicesDest)) {
        New-Item -ItemType Directory -Path $voicesDest -Force | Out-Null
    }
    $voiceFiles = Get-ChildItem -Path $voicesSrc
    foreach ($v in $voiceFiles) {
        $dest = Join-Path $voicesDest $v.Name
        Copy-Item $v.FullName -Destination $dest -Force
        Write-Host "  -> Copied voice $($v.Name)" -ForegroundColor Green
    }
}

Write-Host "`n================================================" -ForegroundColor Cyan
Write-Host "  PACKAGE COMPLETE! Standalone folder is ready: " -ForegroundColor Green
Write-Host "  $OutputDir" -ForegroundColor White
Write-Host "================================================" -ForegroundColor Cyan
Write-Host "You can now run 'twitch-tts.exe' directly from '$OutputDir' or move the '$OutputDir' folder anywhere!" -ForegroundColor Yellow
