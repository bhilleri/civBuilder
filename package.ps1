# Fabrique le paquet à distribuer : compile la version optimisée, puis rassemble
# l'exécutable, le dossier assets (icônes, licence de la police), le README et le journal
# des versions dans CivBuilder-<version>.zip.
# Usage (PowerShell, à la racine du projet) : .\package.ps1
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.+)"').Matches[0].Groups[1].Value
$name = "CivBuilder-$version"

Write-Host "Compilation de la version optimisée $version..."
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "La compilation a échoué." }

if (Test-Path $name) { Remove-Item -Recurse -Force $name }
New-Item -ItemType Directory $name | Out-Null
Copy-Item target\release\civbuilder.exe $name\
Copy-Item -Recurse assets $name\
Copy-Item README.md, CHANGELOG.md $name\

if (Test-Path "$name.zip") { Remove-Item -Force "$name.zip" }
Compress-Archive -Path $name -DestinationPath "$name.zip"
$size = [math]::Round((Get-Item "$name.zip").Length / 1MB, 1)
Write-Host "Paquet prêt : $name.zip ($size Mo). Décompresser puis lancer civbuilder.exe."
