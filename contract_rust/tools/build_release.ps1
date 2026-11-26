$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root "dist"
$output = Join-Path $dist "ContractForge.exe"

Push-Location $root
try {
    $metadata = cargo metadata --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $targetRoot = (ConvertFrom-Json $metadata).target_directory
    $target = Join-Path $targetRoot "release/ContractForge.exe"
    cargo build --release --locked -p cf-app
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    if (-not (Test-Path -LiteralPath $target -PathType Leaf)) {
        throw "Không tìm thấy binary release: $target"
    }
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    $config = Join-Path $dist "ContractForge.ini"
    $repoDatabase = Join-Path (Split-Path -Parent $root) "data/contract_manager.db"
    if ((Test-Path -LiteralPath $repoDatabase -PathType Leaf) -and
        -not (Test-Path -LiteralPath $config -PathType Leaf)) {
        Set-Content -LiteralPath $config -Value @(
            "# Thư mục dữ liệu tương đối với vị trí ContractForge.exe",
            "data_root=..\..\data"
        ) -Encoding utf8
    }
    Copy-Item -LiteralPath $target -Destination $output -Force
    $file = Get-Item -LiteralPath $output
    Write-Output "Đã tạo $($file.FullName) ($($file.Length) bytes)"
}
finally {
    Pop-Location
}
