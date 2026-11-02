#!/usr/bin/env bash
# Tourne les GIF des animations du jeu (déplacements, combats, fin de tour, villages, camp)
# en 2D et en 3D, pour docs/ANIMATIONS.md. Il faut ffmpeg.
#
#   scripts/animations.sh                    # toutes les scènes, dans docs/animations
#   SCENES=tir,camp scripts/animations.sh    # seulement ces scènes
#   scripts/animations.sh /tmp/essai         # autre dossier
#
# Sans écran (serveur, CI) : lancé sous xvfb-run s'il est installé.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v ffmpeg >/dev/null || { echo "ffmpeg est nécessaire pour assembler les GIF" >&2; exit 1; }
export CIVBUILDER_SEED="${CIVBUILDER_SEED:-777}"
export CIVBUILDER_CAPTURE="${1:-docs/animations}"
export CIVBUILDER_CAPTURE_GIF="${SCENES:-1}"
if [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] && command -v xvfb-run >/dev/null; then
    exec xvfb-run -a -s "-screen 0 1600x1000x24" cargo run --release --no-default-features
fi
exec cargo run --release
