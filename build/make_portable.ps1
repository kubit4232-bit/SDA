param(
    [string]$Root = "E:\site\IDM\SDA",
    [string]$OutName = "SDA-Portable-x64"
)

$ErrorActionPreference = "Stop"
$bin = Join-Path $Root "target\release"
$outDir = Join-Path $Root "build\output\$OutName"

if (Test-Path $outDir) { Remove-Item $outDir -Recurse -Force }
New-Item -ItemType Directory -Path $outDir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $outDir "resources\icons") -Force | Out-Null

foreach ($f in @("sda.exe", "sda-gui.exe", "msg_host.exe", "yt-dlp.exe")) {
    $src = Join-Path $bin $f
    if (-not (Test-Path $src)) { throw "Missing build artifact: $src" }
    Copy-Item $src -Destination $outDir -Force
}

$icons = Join-Path $Root "resources\icons\*.svg"
Copy-Item $icons -Destination (Join-Path $outDir "resources\icons") -Force
Copy-Item (Join-Path $Root "resources\app.ico") -Destination $outDir -Force

Set-Content -Path (Join-Path $outDir "portable.txt") -Encoding ASCII -Value "SDA portable mode: settings.json and jobs.json are stored in this folder."
$readme = @(
    "Super Download Agent (SDA) - portable build",
    "",
    "Run sda-gui.exe for the IDM-style GUI, or sda.exe for the CLI.",
    "Settings (settings.json) and the downloads list (jobs.json) are stored",
    "next to these executables, because portable.txt is present.",
    "Delete portable.txt to use %LOCALAPPDATA%\SDA instead.",
    "yt-dlp.exe is used for video sites."
)
Set-Content -Path (Join-Path $outDir "README.txt") -Value $readme -Encoding ASCII

$zip = Join-Path $Root "build\output\$OutName.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $outDir -DestinationPath $zip -CompressionLevel Optimal

Get-ChildItem $outDir -Recurse -File | Select-Object FullName, Length
Write-Output "PORTABLE_ZIP=$zip"
