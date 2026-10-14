$ErrorActionPreference = 'Stop'
$meshgraphRoot = Split-Path -Parent $PSScriptRoot
$meshgraphBinary = Join-Path $meshgraphRoot 'target/release/hyperion_bridge.exe'
$meshgraphSetup = Join-Path $meshgraphRoot 'public/downloads/MeshGraphLeapSetup.exe'
$meshgraphRuntime = $env:HYPERION_LEAPC_PATH
if (!$meshgraphRuntime) { $meshgraphRuntime = 'C:\Program Files\Ultraleap\LeapSDK\lib\x64\LeapC.dll' }
if (!(Test-Path -LiteralPath $meshgraphRuntime)) { throw 'Hyperion is required for this native recovery check' }
$meshgraphProbe = [Net.Sockets.TcpClient]::new()
try { $meshgraphProbe.Connect('127.0.0.1',6437); throw 'An existing bridge is running; stop this check to preserve it' }
catch [Net.Sockets.SocketException] { } finally { $meshgraphProbe.Dispose() }
$meshgraphFixtureDir = Join-Path $meshgraphRoot ("releases/leap-recovery-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $meshgraphFixtureDir | Out-Null
$meshgraphFixtureDll = Join-Path $meshgraphFixtureDir 'LeapC.dll'
$meshgraphProcesses = [Collections.Generic.List[Diagnostics.Process]]::new()
function Start-FailedBridge {
    [IO.File]::WriteAllBytes($meshgraphFixtureDll, [byte[]]@(0))
    $meshgraphStart = [Diagnostics.ProcessStartInfo]::new($meshgraphBinary)
    $meshgraphStart.UseShellExecute = $false
    $meshgraphStart.CreateNoWindow = $true
    $meshgraphStart.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $meshgraphStart.EnvironmentVariables['HYPERION_LEAPC_PATH'] = $meshgraphFixtureDll
    $meshgraphProcess = [Diagnostics.Process]::Start($meshgraphStart)
    $meshgraphProcesses.Add($meshgraphProcess)
    return $meshgraphProcess
}
function Wait-BridgeState([bool] $meshgraphExpectError) {
    for ($meshgraphAttempt = 0; $meshgraphAttempt -lt 80; $meshgraphAttempt++) {
        try {
            $meshgraphStatus = Invoke-RestMethod -Uri 'http://127.0.0.1:6437/health' -TimeoutSec 1
            if ($meshgraphExpectError -and $meshgraphStatus.state -eq 'error') { return }
            if (!$meshgraphExpectError -and ($meshgraphStatus.state -in @('connected','device-connected') -or $meshgraphStatus.type -eq 'frame')) { return }
        } catch { }
        Start-Sleep -Milliseconds 200
    }
    throw 'The bridge did not reach the expected tracking state'
}
try {
    $meshgraphAutomatic = Start-FailedBridge
    Wait-BridgeState $true
    Copy-Item -LiteralPath $meshgraphRuntime -Destination $meshgraphFixtureDll -Force
    Wait-BridgeState $false
    if ($meshgraphAutomatic.HasExited) { throw 'Automatic recovery replaced the process instead of retrying tracking' }
    Write-Output 'Driver failure recovered automatically in the same background process.'
    Stop-Process -Id $meshgraphAutomatic.Id
    $meshgraphAutomatic.WaitForExit()

    $meshgraphFailed = Start-FailedBridge
    Wait-BridgeState $true
    $meshgraphAssembly = [Reflection.Assembly]::LoadFile($meshgraphSetup)
    $meshgraphFlags = [Reflection.BindingFlags]::NonPublic -bor [Reflection.BindingFlags]::Static
    $meshgraphMethod = $meshgraphAssembly.GetType('LeapCompanion').GetMethod('StartBridge', $meshgraphFlags)
    $meshgraphRestarted = $meshgraphMethod.Invoke($null, [object[]]@("$meshgraphBinary", "$meshgraphRuntime"))
    if ($null -eq $meshgraphRestarted) { throw 'The failed bridge was treated as healthy' }
    $meshgraphProcesses.Add($meshgraphRestarted)
    Wait-BridgeState $false
    if (!$meshgraphFailed.HasExited) { throw 'The failed process remained running after app-driven restart' }
    $meshgraphHealthy = $meshgraphMethod.Invoke($null, [object[]]@("$meshgraphBinary", "$meshgraphRuntime"))
    if ($null -ne $meshgraphHealthy -or $meshgraphRestarted.HasExited) { throw 'A healthy bridge was restarted' }
    Write-Output 'Companion restarted the failed bridge with corrected settings and preserved a healthy bridge.'
} finally {
    foreach ($meshgraphProcess in $meshgraphProcesses) {
        if (!$meshgraphProcess.HasExited -and $meshgraphProcess.MainModule.FileName -eq $meshgraphBinary) {
            Stop-Process -Id $meshgraphProcess.Id
            $meshgraphProcess.WaitForExit()
        }
        $meshgraphProcess.Dispose()
    }
    Remove-Item -LiteralPath $meshgraphFixtureDll -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $meshgraphFixtureDir
}
