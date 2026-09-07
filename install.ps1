[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repository = "tu-tu-op/GitSama"
$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$manifestPath = Join-Path $scriptDirectory "Cargo.toml"
$version = "0.1.0"
if (Test-Path $manifestPath) {
    $versionLine = Select-String -Path $manifestPath -Pattern '^version = "([^"]+)"' | Select-Object -First 1
    if ($versionLine) {
        $version = $versionLine.Matches[0].Groups[1].Value
    }
}

try {
    $gitVersionText = (& git --version 2>&1 | Out-String).Trim()
} catch {
    throw "GitSama needs Git 2.54 or newer. Git was not found."
}
if ($gitVersionText -notmatch '(\d+)\.(\d+)\.(\d+)') {
    throw "Could not read the installed Git version. Detected: $gitVersionText"
}
$gitMajor = [int]$Matches[1]
$gitMinor = [int]$Matches[2]
if (($gitMajor -lt 2) -or (($gitMajor -eq 2) -and ($gitMinor -lt 54))) {
    throw "GitSama needs Git 2.54 or newer. Detected: $gitVersionText. Upgrade Git and run install.ps1 again."
}

$installRoot = Join-Path $env:USERPROFILE ".gitsama"
$binDirectory = Join-Path $installRoot "bin"
$source = Join-Path $scriptDirectory "target\release\gitsama.exe"
$tempDirectory = $null
try {
    if (-not (Test-Path $source)) {
        $cargo = Get-Command cargo -ErrorAction SilentlyContinue
        if ($cargo) {
            Write-Host "Building GitSama from source..."
            & cargo build --locked --release --manifest-path $manifestPath
            if ($LASTEXITCODE -ne 0) {
                throw "Cargo could not build GitSama."
            }
        } else {
            if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne "X64") {
                throw "No Windows release asset is available for this architecture. Install Rust and Cargo, then retry."
            }
            $asset = "gitsama-v$version-windows-x86_64.zip"
            $tempDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("gitsama-" + [Guid]::NewGuid())
            New-Item -ItemType Directory -Path $tempDirectory | Out-Null
            $archive = Join-Path $tempDirectory $asset
            $url = "https://github.com/$repository/releases/download/v$version/$asset"
            Write-Host "Downloading $url"
            Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $archive
            $checksumPath = "$archive.sha256"
            Invoke-WebRequest -UseBasicParsing -Uri "$url.sha256" -OutFile $checksumPath
            $expectedHash = (Get-Content -LiteralPath $checksumPath).Trim().Split()[0]
            $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash
            if ($expectedHash -ine $actualHash) {
                throw "The downloaded GitSama archive failed its SHA-256 checksum."
            }
            Expand-Archive -LiteralPath $archive -DestinationPath $tempDirectory
            $source = Join-Path $tempDirectory "gitsama.exe"
        }
    }

    if (-not (Test-Path $source)) {
        throw "GitSama binary was not produced. Check the build output and try again."
    }

    New-Item -ItemType Directory -Force -Path $binDirectory | Out-Null
    $target = Join-Path $binDirectory "gitsama.exe"
    Copy-Item -LiteralPath $source -Destination $target -Force

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $entries = @()
    if ($userPath) {
        $entries = $userPath -split ';' | Where-Object { $_ -and $_.Trim() }
    }
    if (-not ($entries | Where-Object { $_.TrimEnd('\') -ieq $binDirectory.TrimEnd('\') })) {
        [Environment]::SetEnvironmentVariable("Path", (($entries + $binDirectory) -join ';'), "User")
        Write-Host "Added $binDirectory to the user PATH."
    }

    $oldNonInteractive = $env:GITSAMA_NONINTERACTIVE
    $env:GITSAMA_NONINTERACTIVE = "1"
    try {
        & $target setup
        if ($LASTEXITCODE -ne 0) {
            throw "GitSama setup failed."
        }
    } finally {
        $env:GITSAMA_NONINTERACTIVE = $oldNonInteractive
    }

    Write-Host ""
    Write-Host "GitSama installed at $target."
    Write-Host "Open a new PowerShell window if gitsama is not immediately on PATH."
} finally {
    if ($tempDirectory -and (Test-Path $tempDirectory)) {
        Remove-Item -LiteralPath $tempDirectory -Recurse -Force
    }
}
