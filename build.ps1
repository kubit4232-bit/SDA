cd /d E:\site\IDM\SDA
cargo build --release
if ($LASTEXITCODE -eq 0) {
    Write-Output "Build successful: target\release\sda.exe"
} else {
    Write-Output "Build failed"
    exit 1
}
