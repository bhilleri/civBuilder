# Amorce le dossier target d'un nouveau worktree en copiant celui du checkout principal :
# les dépendances déjà compilées (Bevy, etc.) sont réutilisées, seul le jeu est recompilé.
# Cargo vérifie tout lui-même : ce qui ne correspond plus (autre Cargo.lock, etc.) est recompilé.
# Fichier enregistré en UTF-8 avec BOM : sans lui, PowerShell 5.1 lit mal les accents.
#
# Lancé tout seul au démarrage d'une session Claude Code (hook de .claude/settings.json).
# À la main, depuis la racine du worktree :
#   powershell -ExecutionPolicy Bypass -File scripts/amorcer-target.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/amorcer-target.ps1 -Source D:\autre\worktree

param(
    # Dossier à copier (par défaut : le checkout principal, sinon le worktree compilé le plus récemment)
    [string]$Source,
    # Mode hook : le worktree à amorcer est lu dans le JSON reçu sur l'entrée standard (champ cwd),
    # et aucun échec n'est remonté, pour ne jamais gêner le démarrage de la session
    [switch]$Hook
)

$ErrorActionPreference = 'Stop'

# Date de la dernière compilation d'un dossier, ou $null s'il n'a pas de target exploitable
function DateDeCompilation($dossier) {
    $empreintes = Join-Path $dossier 'target\debug\.fingerprint'
    if (Test-Path $empreintes) { (Get-Item $empreintes).LastWriteTime } else { $null }
}

# Cargo garde .cargo-lock ouvert tant qu'il compile : l'ouvrir en exclusif échoue alors
function CompilationEnCours($dossier) {
    foreach ($profil in 'debug', 'release') {
        $verrou = Join-Path $dossier "target\$profil\.cargo-lock"
        if (-not (Test-Path $verrou)) { continue }
        try {
            [IO.File]::Open($verrou, 'Open', 'Read', 'None').Close()
        } catch {
            return $true
        }
    }
    return $false
}

function CheminWindows($chemin) {
    [IO.Path]::GetFullPath(($chemin -replace '/', '\')).TrimEnd('\')
}

try {
    if ($Hook) {
        $depart = ([Console]::In.ReadToEnd() | ConvertFrom-Json).cwd
        $gitDir = git -C $depart rev-parse --path-format=absolute --git-dir
        $gitCommun = git -C $depart rev-parse --path-format=absolute --git-common-dir
        # Hors d'un dépôt git, il n'y a rien à amorcer non plus
        if (-not $gitDir) { exit 0 }
        # Dans le checkout principal, il n'y a rien à amorcer
        if ((CheminWindows $gitDir) -eq (CheminWindows $gitCommun)) { exit 0 }
        $racine = CheminWindows (git -C $depart rev-parse --show-toplevel)
    } else {
        $racine = CheminWindows (Join-Path $PSScriptRoot '..')
    }
    $cible = Join-Path $racine 'target'

    if (Test-Path (Join-Path $cible 'debug\.fingerprint')) {
        if (-not $Hook) { Write-Output "Rien à faire : $cible contient déjà une compilation." }
        exit 0
    }

    if ($Source) {
        if (-not (DateDeCompilation $Source)) { throw "Pas de target\debug exploitable dans $Source" }
    } else {
        # Le premier worktree listé par git est le checkout principal
        $voisins = @(git -C $racine worktree list --porcelain |
            Where-Object { $_ -like 'worktree *' } |
            ForEach-Object { CheminWindows ($_ -replace '^worktree ', '') } |
            Where-Object { $_ -ne $racine -and (DateDeCompilation $_) })
        $principal = CheminWindows (git -C $racine worktree list --porcelain | Select-Object -First 1 | ForEach-Object { $_ -replace '^worktree ', '' })
        if ($voisins -contains $principal) {
            $Source = $principal
        } elseif ($voisins.Count -gt 0) {
            $Source = $voisins | Sort-Object { DateDeCompilation $_ } -Descending | Select-Object -First 1
        } else {
            Write-Output 'Aucun target à copier : la première compilation sera complète.'
            exit 0
        }
    }

    # Une copie pendant une compilation pourrait prendre des fichiers à moitié écrits :
    # on laisse 30 s à une vérification courte de rust-analyzer, puis on renonce
    $attente = [Diagnostics.Stopwatch]::StartNew()
    while (CompilationEnCours $Source) {
        if ($attente.Elapsed.TotalSeconds -ge 30) {
            Write-Output "Compilation en cours dans $Source : target non copié, la première compilation sera complète."
            exit 0
        }
        Start-Sleep -Seconds 2
    }

    Write-Output "Copie de $Source\target vers ${cible}..."
    $chrono = [Diagnostics.Stopwatch]::StartNew()
    # Sans le cache incrémental (lié à l'emplacement du code) ni l'exécutable du jeu
    # (il peut être en cours d'exécution, et sera refait de toute façon)
    foreach ($profil in 'debug', 'release') {
        $dossierProfil = Join-Path $Source "target\$profil"
        if (-not (Test-Path $dossierProfil)) { continue }
        robocopy $dossierProfil (Join-Path $cible $profil) /E /MT:16 /R:1 /W:1 /NFL /NDL /NJH /NJS /NP `
            /XD incremental examples /XF civbuilder.exe civbuilder.pdb civbuilder.d | Out-Null
        # Robocopy : un code de 0 à 7 indique une réussite, 8 ou plus une erreur
        if ($LASTEXITCODE -ge 8) { throw "Échec de la copie de $profil (robocopy, code $LASTEXITCODE)" }
    }
    Write-Output ("Copie terminée en {0:N0} s. La prochaine compilation ne refait que ce qui a changé." -f $chrono.Elapsed.TotalSeconds)
    exit 0
} catch {
    if ($Hook) {
        Write-Output "Amorçage du target ignoré : $_"
        exit 0
    }
    throw
}
