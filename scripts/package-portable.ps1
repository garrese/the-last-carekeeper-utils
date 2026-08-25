param(
    [string]$Version = "0.1.3",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$portableRoot = Join-Path $projectRoot "portable"
$packagePrefix = "The Last Carekeeper Utils-"
$packageRoot = Join-Path $portableRoot "$packagePrefix$Version"
$releaseExe = Join-Path $projectRoot "src-tauri\target\release\the-last-carekeeper-utils.exe"
$portableDataFiles = @("Food.csv", "Memories.csv", "Humans.csv", "asset-mappings.json")

if (-not (Test-Path -LiteralPath $portableRoot)) {
    New-Item -ItemType Directory -Path $portableRoot | Out-Null
}

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

$targetVersion = [version]$Version
$previousPackage = Get-ChildItem -LiteralPath $portableRoot -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name.StartsWith($packagePrefix) } |
    ForEach-Object {
        $candidateText = $_.Name.Substring($packagePrefix.Length)
        try {
            $candidateVersion = [version]$candidateText
            if ($candidateVersion -lt $targetVersion) {
                [pscustomobject]@{ Directory = $_; Version = $candidateVersion }
            }
        }
        catch {
            Write-Warning "Ignoring portable folder with an invalid version: $($_.Name)"
        }
    } |
    Sort-Object Version -Descending |
    Select-Object -First 1

$previousDataRoot = $null
if ($null -ne $previousPackage) {
    $previousDataRoot = Join-Path $previousPackage.Directory.FullName "data"
    $missingFiles = $portableDataFiles | Where-Object { -not (Test-Path -LiteralPath (Join-Path $previousDataRoot $_)) }
    if ($missingFiles.Count -gt 0) {
        throw "Previous portable data is incomplete in $previousDataRoot. Missing: $($missingFiles -join ', ')"
    }
}

New-Item -ItemType Directory -Path $packageRoot | Out-Null
Copy-Item -LiteralPath $releaseExe -Destination $packageRoot
Copy-Item -LiteralPath (Join-Path $projectRoot "data") -Destination $packageRoot -Recurse
Copy-Item -LiteralPath (Join-Path $projectRoot "README.md") -Destination $packageRoot

if ($null -ne $previousDataRoot) {
    foreach ($fileName in $portableDataFiles) {
        Copy-Item -LiteralPath (Join-Path $previousDataRoot $fileName) -Destination (Join-Path $packageRoot "data\$fileName") -Force
    }
    Write-Host "Editable data migrated from: $($previousPackage.Directory.FullName)"
}

Write-Host "Portable package created at: $packageRoot"
