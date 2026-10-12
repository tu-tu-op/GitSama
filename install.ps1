[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repository = "tu-tu-op/GitSama"
$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$manifestPath = Join-Path $scriptDirectory "Cargo.toml"
$version = "0.1.2"
if (Test-Path -LiteralPath $manifestPath) {
    $versionLine = Select-String -LiteralPath $manifestPath -Pattern '^version = "([^"]+)"' | Select-Object -First 1
    if ($versionLine) {
        $version = $versionLine.Matches[0].Groups[1].Value
    }
}
if ($version -notmatch '\A[0-9]+\.[0-9]+\.[0-9]+\z') {
    throw "GitSama version must be a stable major.minor.patch version."
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
$tempBase = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\')
try {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if ((Test-Path -LiteralPath $manifestPath) -and $cargo) {
        Write-Host "Building GitSama from source..."
        & cargo build --locked --release --manifest-path $manifestPath
        if ($LASTEXITCODE -ne 0) {
            throw "Cargo could not build GitSama."
        }
    } elseif (-not (Test-Path -LiteralPath $source)) {
        if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne "X64") {
            throw "No Windows release asset is available for this architecture. Install Rust and Cargo, then retry."
        }
        $asset = "gitsama-v$version-windows-x86_64.zip"
        $tempDirectory = Join-Path $tempBase ("gitsama-" + [Guid]::NewGuid())
        New-Item -ItemType Directory -Path $tempDirectory | Out-Null
        $archive = Join-Path $tempDirectory $asset
        $url = "https://github.com/$repository/releases/download/v$version/$asset"
        Write-Host "Downloading $url"
        Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $archive -TimeoutSec 120
        $checksumPath = "$archive.sha256"
        Invoke-WebRequest -UseBasicParsing -Uri "$url.sha256" -OutFile $checksumPath -TimeoutSec 30
        if ((Get-Item -LiteralPath $archive).Length -gt 104857600 -or (Get-Item -LiteralPath $checksumPath).Length -gt 1024) {
            throw "Release download exceeds its size limit."
        }
        $checksumText = (Get-Content -Raw -LiteralPath $checksumPath).Trim()
        if ($checksumText -notmatch ('\A([a-fA-F0-9]{64})\s+\*?' + [regex]::Escape($asset) + '\z')) {
            throw "Invalid checksum for the expected release archive."
        }
        $expectedHash = $Matches[1]
        $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash
        if ($expectedHash -ine $actualHash) {
            throw "The downloaded GitSama archive failed its SHA-256 checksum."
        }
        $source = Join-Path $tempDirectory "gitsama.exe"
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $zip = [System.IO.Compression.ZipFile]::OpenRead($archive)
        try {
            if ($zip.Entries.Count -ne 1 -or $zip.Entries[0].FullName -cne "gitsama.exe" -or $zip.Entries[0].Length -le 0 -or $zip.Entries[0].Length -gt 104857600) {
                throw "Release archive must contain only the bounded gitsama.exe executable."
            }
            $input = $zip.Entries[0].Open()
            $output = [System.IO.File]::Open($source, [System.IO.FileMode]::CreateNew)
            try {
                $buffer = New-Object byte[] 65536
                $written = 0L
                while (($count = $input.Read($buffer, 0, $buffer.Length)) -gt 0) {
                    $written += $count
                    if ($written -gt 104857600) { throw "Extracted binary exceeds its size limit." }
                    $output.Write($buffer, 0, $count)
                }
            } finally { $input.Dispose(); $output.Dispose() }
        } finally { $zip.Dispose() }
    }

    if (-not (Test-Path -LiteralPath $source)) {
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
    if ($tempDirectory -and (Test-Path -LiteralPath $tempDirectory)) {
        $cleanupPath = (Resolve-Path -LiteralPath $tempDirectory).ProviderPath
        if ([System.IO.Path]::GetDirectoryName($cleanupPath) -ine $tempBase -or (((Get-Item -LiteralPath $cleanupPath).Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
            throw "Refusing cleanup outside the installer temporary directory."
        }
        Remove-Item -LiteralPath $cleanupPath -Recurse -Force
    }
}
