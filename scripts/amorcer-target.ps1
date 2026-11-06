# Amorce le dossier target d'un nouveau worktree en copiant celui d'un autre worktree :
# les dépendances déjà compilées (Bevy, etc.) sont réutilisées, seul le jeu est recompilé.
# Cargo vérifie tout lui-même : ce qui ne correspond plus (autre Cargo.lock, etc.) est recompilé.
# Fichier enregistré en UTF-8 avec BOM : sans lui, PowerShell 5.1 lit mal les accents.
#
# Usage, depuis la racine du worktree :
#   powershell -ExecutionPolicy Bypass -File scripts/amorcer-target.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/amorcer-target.ps1 -Source D:\autre\worktree

param(
    # Dossier du worktree à copier (par défaut : le plus récemment compilé parmi les voisins)
    [string]$Source
)

$ErrorActionPreference = 'Stop'
$racine = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$cible = Join-Path $racine 'target'

if (Test-Path (Join-Path $cible 'debug\.fingerprint')) {
    Write-Output "Rien à faire : $cible contient déjà une compilation."
    exit 0
}

# Date de la dernière compilation d'un worktree, ou $null s'il n'a pas de target exploitable
function DateDeCompilation($dossier) {
    $empreintes = Join-Path $dossier 'target\debug\.fingerprint'
    if (Test-Path $empreintes) { (Get-Item $empreintes).LastWriteTime } else { $null }
}

if ($Source) {
    if (-not (DateDeCompilation $Source)) { throw "Pas de target\debug exploitable dans $Source" }
} else {
    # Les worktrees sont listés par git : le dépôt principal en fait partie
    $voisins = git -C $racine worktree list --porcelain 2>$null |
        Where-Object { $_ -like 'worktree *' } |
        ForEach-Object { ($_ -replace '^worktree ', '') -replace '/', '\' } |
        Where-Object { $_ -ne $racine -and (DateDeCompilation $_) }
    if (-not $voisins) {
        Write-Output 'Aucun worktree à copier : la première compilation sera complète.'
        exit 0
    }
    $Source = $voisins | Sort-Object { DateDeCompilation $_ } -Descending | Select-Object -First 1
}

Write-Output "Copie de $Source\target vers ${cible}..."
$chrono = [Diagnostics.Stopwatch]::StartNew()
# Sans le cache incrémental (lié à l'emplacement du code) ni l'exécutable du jeu
# (il peut être en cours d'exécution, et sera refait de toute façon)
foreach ($profil in 'debug', 'release') {
    $depart = Join-Path $Source "target\$profil"
    if (-not (Test-Path $depart)) { continue }
    robocopy $depart (Join-Path $cible $profil) /E /MT:16 /R:1 /W:1 /NFL /NDL /NJH /NJS /NP `
        /XD incremental examples /XF civbuilder.exe civbuilder.pdb civbuilder.d | Out-Null
    # Robocopy : un code de 0 à 7 indique une réussite, 8 ou plus une erreur
    if ($LASTEXITCODE -ge 8) { throw "Échec de la copie de $profil (robocopy, code $LASTEXITCODE)" }
}
Write-Output ("Copie terminée en {0:N0} s. La prochaine compilation ne refait que ce qui a changé." -f $chrono.Elapsed.TotalSeconds)
