$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    New-Item -ItemType Directory -Force 'build/classes', 'dist' | Out-Null
    & javac --release 8 -encoding UTF-8 -d build/classes src/main/java/dev/echo/agent/EchoAgent.java
    if ($LASTEXITCODE -ne 0) { throw 'Echo Agent compilation failed (build requires JDK 9+)' }
    $manifest = "Manifest-Version: 1.0`r`nPremain-Class: dev.echo.agent.EchoAgent`r`nCan-Redefine-Classes: true`r`n`r`n"
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot 'build/MANIFEST.MF'), $manifest, [Text.Encoding]::ASCII)
    & jar cfm dist/echo-agent.jar build/MANIFEST.MF -C build/classes .
    if ($LASTEXITCODE -ne 0) { throw 'Echo Agent JAR packaging failed' }
} finally { Pop-Location }
