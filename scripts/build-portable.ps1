param(
    [string]$Version = "1.7.2",
    [string]$TargetDir = "V:\target\x86_64-pc-windows-gnu\release",
    [string]$OutDir = "V:\releases"
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $TargetDir "trasea.exe"
if (-not (Test-Path $exe)) {
    throw "trasea.exe not found at $exe. Build release first."
}

$stage = Join-Path $env:TEMP ("trasea-portable-" + $Version + "-" + [guid]::NewGuid().ToString("N").Substring(0,8))
New-Item -ItemType Directory -Force -Path $stage | Out-Null
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

try {
    Copy-Item $exe (Join-Path $stage "trasea.exe") -Force
    Set-Content -Path (Join-Path $stage "portable.flag") -Value "portable=true" -Encoding ASCII
    Copy-Item (Join-Path $repoRoot "src-tauri\resources\realesrgan") (Join-Path $stage "realesrgan") -Recurse -Force
    Copy-Item (Join-Path $repoRoot "src-tauri\resources\sample") (Join-Path $stage "sample") -Recurse -Force
    Copy-Item (Join-Path $repoRoot "THIRD_PARTY_LICENSES.md") (Join-Path $stage "THIRD_PARTY_LICENSES.md") -Force
    Copy-Item (Join-Path $repoRoot "LICENSE") (Join-Path $stage "LICENSE") -Force

    $zipPath = Join-Path $OutDir ("Trasea_" + $Version + "_x64-portable.zip")
    if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
    Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zipPath -CompressionLevel Optimal
    Write-Output "Created portable package: $zipPath"
    Get-FileHash $zipPath -Algorithm SHA256 | Format-List Path, Hash
} finally {
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue }
}
