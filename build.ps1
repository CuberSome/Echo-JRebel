$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    $manifest = Get-Content -LiteralPath 'plugin.toml' -Raw
    if ($manifest -notmatch '(?m)^version\s*=\s*"([^"]+)"\s*$') {
        throw 'Could not read plugin version from plugin.toml'
    }
    $version = $Matches[1]

    & ./agent/build.ps1
    if ($LASTEXITCODE -ne 0) { throw 'Echo Agent build failed' }

    rustup target add wasm32-wasip2
    if ($LASTEXITCODE -ne 0) { throw 'Could not install wasm32-wasip2 target' }

    cargo test --locked
    if ($LASTEXITCODE -ne 0) { throw 'Rust checks failed' }

    cargo build --locked --release --target wasm32-wasip2
    if ($LASTEXITCODE -ne 0) { throw 'WASM build failed' }

    $targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $PSScriptRoot 'target' }
    $wasm = Join-Path $targetDir 'wasm32-wasip2/release/openidea_jrebel_integration.wasm'
    if (!(Test-Path -LiteralPath $wasm)) { throw "Build output not found: $wasm" }

    New-Item -ItemType Directory -Force -Path 'dist' | Out-Null
    Copy-Item -LiteralPath $wasm -Destination 'dist/plugin.wasm' -Force
    Copy-Item -LiteralPath 'agent/dist/echo-agent.jar' -Destination 'dist/echo-agent.jar' -Force
    Copy-Item -LiteralPath 'vendor/openidea-plugin/LICENSE' -Destination 'dist/OPENIDEA-SDK-LICENSE' -Force

    $releaseDir = Join-Path $PSScriptRoot 'release'
    New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
    $zip = Join-Path $releaseDir "echo-jrebel-$version.zip"
    $package = Join-Path $releaseDir "echo-jrebel-$version.oiplugin"
    Remove-Item -LiteralPath $zip -Force -ErrorAction SilentlyContinue
    Compress-Archive -LiteralPath 'plugin.toml', 'settings.schema.json', 'dist', 'README.md', 'VERIFICATION.md', 'docs', 'THIRD_PARTY_NOTICES.md' -DestinationPath $zip -Force
    Move-Item -LiteralPath $zip -Destination $package -Force
    Copy-Item -LiteralPath 'dist/echo-agent.jar' -Destination (Join-Path $releaseDir 'echo-agent.jar') -Force
    Write-Output "Package created: $package"
} finally {
    Pop-Location
}
