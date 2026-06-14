param(
    [string]$VideoPath = "demo/graphmesh-demo.webm",
    [string]$OutputPath = "demo/graphmesh-demo-narrated.mp4"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$video = Join-Path $root $VideoPath
$output = Join-Path $root $OutputPath
$audio = Join-Path $root "demo/graphmesh-narration.wav"

Add-Type -AssemblyName System.Speech
$speech = [System.Speech.Synthesis.SpeechSynthesizer]::new()
$speech.SelectVoice("Microsoft David Desktop")
$speech.Rate = -1
$speech.Volume = 100
$speech.SetOutputToWaveFile($audio)
$speech.Speak(@"
GraphMesh is an interactive spatial knowledge graph built with Rust, WebAssembly, Oxigraph, and Web XR.
The scene supports mouse look, keyboard movement, and scroll zoom for exploring the model.
Selecting the rover creates an exploded component view.
Each selectable mesh is connected to semantic metadata stored in Oxigraph.
The interface displays the active SPARQL query, graph hierarchy, and relationships between connected components.
Related parts are highlighted directly in the three dimensional scene.
The dedicated Assemble Rover control returns the model to its complete form.
"@)
$speech.Dispose()

$ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
if (-not $ffmpeg) {
    $ffmpeg = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Recurse -Filter ffmpeg.exe |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $ffmpeg) {
    throw "FFmpeg is required to combine the demo video and narration."
}

& $ffmpeg -y -i $video -i $audio -vf "tpad=stop_mode=clone:stop_duration=2" -c:v libx264 -preset medium -crf 20 -c:a aac -b:a 160k -shortest $output
Write-Output $output
