#!/usr/bin/env bash
# Screenshots the standalone panel. Targets the window by address and refuses
# to act unless the focus actually landed on it -- a title selector that finds
# nothing falls back to whatever is focused, which is how a terminal ended up
# floated across the screen.
set -euo pipefail
out=${1:?usage: shot.sh <output.png> [dpi-scale]}
dpi=${2:-1}

pkill -x pulteqfx 2>/dev/null || true
sleep 0.5
XDG_CONFIG_HOME="${SHOT_CONFIG:-$HOME/.config}" ./target/release/pulteqfx --backend dummy --dpi-scale "$dpi" >/dev/null 2>&1 &
sleep 3

addr=$(hyprctl clients -j | python3 -c "
import json,sys
for c in json.load(sys.stdin):
    if c['title'] == 'PultEQFx':
        print(c['address']); break
")
[ -n "$addr" ] || { echo 'no PultEQFx window'; exit 1; }

hyprctl repl "return hl.dispatch(hl.dsp.focus({ window = \"address:$addr\" }))" >/dev/null
sleep 0.5
active=$(hyprctl activewindow -j | python3 -c "import json,sys; print(json.load(sys.stdin)['address'])")
[ "$active" = "$addr" ] || { echo 'focus did not land; refusing to dispatch'; exit 1; }

floating=$(hyprctl clients -j | python3 -c "
import json, sys
addr = sys.argv[1]
print(next(c['floating'] for c in json.load(sys.stdin) if c['address'] == addr))
" "$addr")
[ "$floating" = "True" ] || hyprctl repl 'return hl.dispatch(hl.dsp.window.float())' >/dev/null
sleep 0.5
hyprctl repl 'return hl.dispatch(hl.dsp.window.move({ x = 100, y = 100 }))' >/dev/null
# The compositor's window opacity lets whatever is behind show through the
# panel, which is fine to look at and ruins a screenshot: the shipped
# doc/panel.png had a terminal legible through the faceplate for a week.
#
# It is done with a window rule and not `hyprctl setprop`, because setprop
# answers "unknown request" to every property name on this Hyprland -- and
# because the two setprop lines that used to be here ended in `|| true`, which
# is how nobody noticed. A rule registered now lands after the ones the config
# registered, and for opacity the last match wins, so this beats Omarchy's
# `default-opacity` tag. It applies to the window already mapped.
hyprctl repl 'return hl.window_rule({ match = { title = "^(PultEQFx)$" }, opacity = "1 1 1" })' >/dev/null
opacity=$(hyprctl getprop "address:$addr" opacity)
[ "$opacity" = "1" ] || { echo "window is $opacity opaque; the panel would show what is behind it"; exit 1; }
sleep 1.5

# By address again, not activewindow: moving the window can hand focus back to
# whatever was under the cursor.
read -r x y w h < <(hyprctl clients -j | python3 -c "
import json, sys
addr = sys.argv[1]
for c in json.load(sys.stdin):
    if c['address'] == addr:
        assert c['title'] == 'PultEQFx', c['title']
        print(c['at'][0], c['at'][1], c['size'][0], c['size'][1])
        break
else:
    raise SystemExit('window gone')
" "$addr")
grim -g "$x,$y ${w}x${h}" "$out"
magick "$out" -format 'wrote %f, %wx%h\n' info:
