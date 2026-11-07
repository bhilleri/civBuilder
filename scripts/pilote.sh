#!/usr/bin/env bash
# Pilote une partie en mode joueur (voir src/pilote.rs), pour les agents de test.
#
# Lancer une partie (l'exécutable tourne en arrière-plan, journal dans <dossier>/jeu.log) :
#   scripts/pilote.sh lancer <dossier> [graine] [carte, ex. 128x74] [fenêtre, ex. 1920x1080]
# Envoyer des ordres et lire les réponses (attend que le jeu les ait tous traités) :
#   scripts/pilote.sh <dossier> "etat" "survol 90 45" "clic-droit 90 45"
#   scripts/pilote.sh <dossier> "aide"
# Fermer la partie :
#   scripts/pilote.sh <dossier> quitter
#
# Variables : PILOTE_EXE (exécutable, par défaut target/debug/civbuilder.exe), PILOTE_ATTENTE
# (secondes d'attente au plus, 900 par défaut : la génération du monde est longue).
set -u

if [ "${1:-}" = "lancer" ]; then
    dossier="${2:?dossier manquant}"
    graine="${3:-777}"
    carte="${4:-}"
    fenetre="${5:-1920x1080}"
    racine="$(cd "$(dirname "$0")/.." && pwd)"
    exe="${PILOTE_EXE:-$racine/target/debug/civbuilder.exe}"
    mkdir -p "$dossier"
    : > "$dossier/ordres.txt"
    : > "$dossier/ordres.reponses.txt"
    # (sans cargo, Bevy chercherait le dossier assets à côté de l'exécutable : icônes manquantes)
    cd "$racine" || exit 1
    export BEVY_ASSET_ROOT="$racine" CIVBUILDER_SEED="$graine" CIVBUILDER_FENETRE="$fenetre" CIVBUILDER_PILOTE="$dossier/ordres.txt"
    if [ -n "$carte" ]; then
        export CIVBUILDER_MAP="$carte"
    fi
    nohup "$exe" > "$dossier/jeu.log" 2>&1 &
    echo "OK partie lancée (processus $!, graine $graine, fenêtre $fenetre), journal : $dossier/jeu.log"
    exit 0
fi

dossier="${1:?dossier manquant}"
shift
reponses="$dossier/ordres.reponses.txt"
[ -f "$reponses" ] || { echo "ERREUR pas de partie lancée dans $dossier"; exit 1; }
avant=$(wc -c < "$reponses")
quitter=non
repere="$$-$RANDOM-$(date +%s)"
for ordre in "$@"; do
    printf '%s\n' "$ordre" >> "$dossier/ordres.txt"
    [ "$ordre" = "quitter" ] && quitter=oui
done
if [ "$quitter" = oui ]; then
    echo "OK fermeture demandée"
    exit 0
fi
printf 'repere %s\n' "$repere" >> "$dossier/ordres.txt"
fin=$(( $(date +%s) + ${PILOTE_ATTENTE:-900} ))
while ! tail -c +$((avant + 1)) "$reponses" | grep -qF "# repere $repere"; do
    if [ "$(date +%s)" -gt "$fin" ]; then
        echo "ERREUR pas de réponse du jeu (fin du journal $dossier/jeu.log) :"
        tail -5 "$dossier/jeu.log"
        exit 1
    fi
    sleep 0.1
done
tail -c +$((avant + 1)) "$reponses" | grep -vF "# repere $repere"
exit 0
