param(
    [string]$Version = "0.1.3",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$packageRoot = Join-Path $projectRoot "portable\The Last Carekeeper Utils-$Version"
$releaseExe = Join-Path $projectRoot "src-tauri\target\release\the-last-carekeeper-utils.exe"

if (Test-Path -LiteralPath $packageRoot) {
    throw "Portable output already exists: $packageRoot"
}

if (-not $SkipBuild) {
    $tauri = Join-Path $projectRoot "node_modules\.bin\tauri.cmd"
    if (-not (Test-Path -LiteralPath $tauri)) {
        throw "Tauri CLI is not installed. Run pnpm install first."
    }

    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    $env:Path = "$cargoBin;$env:Path"
    Push-Location $projectRoot
    try {
        & $tauri build --no-bundle
        if ($LASTEXITCODE -ne 0) {
            throw "Tauri build failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}

if (-not (Test-Path -LiteralPath $releaseExe)) {
    throw "Release executable not found: $releaseExe"
}

New-Item -ItemType Directory -Path $packageRoot | Out-Null
Copy-Item -LiteralPath $releaseExe -Destination $packageRoot
Copy-Item -LiteralPath (Join-Path $projectRoot "data") -Destination $packageRoot -Recurse
Copy-Item -LiteralPath (Join-Path $projectRoot "README.md") -Destination $packageRoot

Write-Host "Portable package created at: $packageRoot"
