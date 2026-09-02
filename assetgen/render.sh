#!/usr/bin/env bash
#
# Regenerates every rendered asset the plugin embeds. Run from anywhere; the
# outputs land in assets/gen. The renderer is deterministic, so re-running it
# without changing assetgen reproduces the same files byte for byte.
#
# The knob is a filmstrip rather than one image that gets rotated: rotating a
# sprite carries its baked lighting round with it, so the highlight would
# travel with the knob instead of staying where the panel light is. The arc
# and the frame count must match KNOB_FRAMES and SWEEP in src/editor.

set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

# 48 frames over the 250 degree sweep. The sweep is negative: a positive angle
# turns the part anticlockwise on screen, while a knob's value grows clockwise,
# so frame 0 has to start at +125 and count down. Get the sign wrong and the
# strip runs backwards -- every knob then rests on its maximum, which is easy
# to miss because the flutes are near enough rotationally symmetric that only
# the index stripe gives it away.
cargo run --release -p assetgen -- \
    --part knob_large \
    --out assets/gen/knob_large.png \
    --size 176 --frames 48 --angle 125 --sweep -250
