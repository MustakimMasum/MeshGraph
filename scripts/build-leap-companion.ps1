$ErrorActionPreference = 'Stop'
$meshgraphRoot = Split-Path -Parent $PSScriptRoot
Push-Location $meshgraphRoot
try {
    cargo build --release --locked --bin hyperion_bridge
    if ($LASTEXITCODE -ne 0) { throw 'Bridge build failed' }
    $meshgraphCompiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
    $meshgraphDownloadDir = Join-Path $meshgraphRoot 'public/downloads'
    New-Item -ItemType Directory -Force -Path $meshgraphDownloadDir | Out-Null
    $meshgraphBridgeBinary = Join-Path $meshgraphRoot 'target\release\hyperion_bridge.exe'
    $meshgraphSetupBinary = Join-Path $meshgraphDownloadDir 'MeshGraphLeapSetup.exe'
    $meshgraphCompanionSource = Join-Path $meshgraphRoot 'deploy\windows\LeapCompanion.cs'
    & $meshgraphCompiler /nologo /target:winexe /platform:x64 /reference:System.Windows.Forms.dll `
        "/resource:$meshgraphBridgeBinary,hyperion_bridge.exe" `
        "/out:$meshgraphSetupBinary" $meshgraphCompanionSource
    if ($LASTEXITCODE -ne 0) { throw 'Companion build failed' }
    $meshgraphManifest = @{
        installer_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $meshgraphSetupBinary).Hash.ToLowerInvariant()
        bridge_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $meshgraphBridgeBinary).Hash.ToLowerInvariant()
        platform = 'Windows x86-64'
    } | ConvertTo-Json
    [IO.File]::WriteAllText((Join-Path $meshgraphDownloadDir 'manifest.json'), $meshgraphManifest, [Text.UTF8Encoding]::new($false))
    Write-Output 'Built public/downloads/MeshGraphLeapSetup.exe'
} finally { Pop-Location }
