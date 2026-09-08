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

# The metal selector knob, from the model rather than drawn.
#
# One frame, not a filmstrip: this knob is a knurled cylinder with no
# indicator on it, so it looks the same at every angle and the widget draws
# the pointer on top at whatever angle the switch is thrown to. That is also
# why it must *not* be rotated -- rotating the sprite would carry the light
# baked into it round with the knob, and the highlight would leave the top
# left corner where the panel's light is.
#
# --scale 1: the model is already in millimetres.
# --fallback aluminium: glTF states base colour, metalness and roughness but
#   has no way to state a finish, so the brushing and the micro-texture come
#   from assetgen's own aluminium.
cargo run --release -p assetgen -- \
    --part glb --glb assets/knob_metal.glb \
    --scale 1.0 --fallback aluminium \
    --out assets/gen/knob_metal.png \
    --size 320 --ss 3 --frames 1 --angle 0 \
    --ao-samples 96 --margin 1.02

# The pilot lamp, twice: once as the model gives it and once with the jewel
# and the lamp core swapped for assetgen's own unlit jewel, which is the same
# red an order of magnitude darker. Two renders rather than one render dimmed
# in the widget, because a dark jewel is not a bright one with less light on
# it: its specular stays where it was while everything else falls away, and
# that is what tells the eye the lamp is off rather than the room.
#
# --scale 1000: this model is in metres.
cargo run --release -p assetgen -- \
    --part glb --glb assets/led_ring.glb \
    --scale 1000.0 --fallback aluminium \
    --map "Red illuminated lamp core=jewel_lamp,Deep red translucent jewel=jewel_lamp" \
    --out assets/gen/lamp_lit.png \
    --size 256 --ss 3 --frames 1 --angle 0 \
    --ao-samples 96 --margin 1.06

cargo run --release -p assetgen -- \
    --part glb --glb assets/led_ring.glb \
    --scale 1000.0 --fallback aluminium \
    --map "Red illuminated lamp core=jewel_dark,Deep red translucent jewel=jewel_dark" \
    --out assets/gen/lamp_dark.png \
    --size 256 --ss 3 --frames 1 --angle 0 \
    --ao-samples 96 --margin 1.06
