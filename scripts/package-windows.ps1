# Packages a Windows build of PlayCraft as an installer (.msi) and a portable zip, named
# like the ArtCraft apps:
#   dist\playcraft-<version>-windows-<arch>.msi
#   dist\playcraft-<version>-windows-<arch>-portable.zip
#
#   powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1 -Target x86_64-pc-windows-msvc -Arch x64
#   (Arch: x64, x86 or arm64.) Build first with: cargo build --release --target <Target>
param(
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][ValidateSet("x64", "x86", "arm64")][string]$Arch
)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$bin = "target\$Target\release"
if (-not (Test-Path "$bin\playcraft.exe")) { throw "Build first: cargo build --release --target $Target" }
if (-not (Test-Path "$bin\engine\libmpv-2.dll")) { throw "The engine folder is missing in $bin; build without PLAYCRAFT_SKIP_ENGINE." }

New-Item -ItemType Directory -Force dist | Out-Null
$stage = "dist\stage-$Arch\PlayCraft"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item "$bin\playcraft.exe" $stage
Copy-Item -Recurse "$bin\engine" "$stage\engine"
Get-ChildItem "$stage\engine" -Force -Filter ".*" | Remove-Item -Force
Copy-Item README.md, LICENSE, NOTICE.md, CHANGELOG.md $stage
Copy-Item -Recurse docs "$stage\docs"

# Portable zip
$name = "playcraft-$version-windows-$Arch"
$zip = "dist\$name-portable.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $stage -DestinationPath $zip

# Licence as RTF for the installer's licence page
$rtf = "dist\stage-$Arch\license.rtf"
$text = (Get-Content LICENSE -Raw) -replace '\\', '\\' -replace '\{', '\{' -replace '\}', '\}'
$text = $text -replace "`r?`n", "\par`n"
Set-Content -Path $rtf -Encoding Ascii -Value ("{\rtf1\ansi\deff0{\fonttbl{\f0 Consolas;}}\fs16 " + $text + "}")

# Installer (WiX Toolset v5: dotnet tool install --global wix --version 5.0.2)
$wix = Get-Command wix -ErrorAction SilentlyContinue
if (-not $wix) {
    $env:PATH += ";$env:USERPROFILE\.dotnet\tools"
    $wix = Get-Command wix -ErrorAction SilentlyContinue
}
if (-not $wix) { throw "WiX is not installed: dotnet tool install --global wix --version 5.0.2" }
& wix extension add -g WixToolset.UI.wixext/5.0.2 | Out-Null
$msi = "dist\$name.msi"
& wix build packaging\windows\playcraft.wxs -arch $Arch -ext WixToolset.UI.wixext `
    -d "Version=$version" -d "Stage=$((Resolve-Path $stage).Path)" -d "License=$((Resolve-Path $rtf).Path)" `
    -o $msi
if ($LASTEXITCODE -ne 0) { throw "wix build failed" }
Remove-Item -Force "dist\$name.wixpdb" -ErrorAction SilentlyContinue

Write-Host "Made $zip and $msi"
