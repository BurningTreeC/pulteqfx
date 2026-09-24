# PultEQFx

A circuit modelled passive program equalizer, built with
[NIH-plug](https://github.com/robbert-vdh/nih-plug). Builds as CLAP and VST3,
plus a standalone application for trying it out.

PultEQFx models the passive equalizer circuit of the classic 1950s tube program
equalizer, the Pultec® EQP-1A. It is not affiliated with, endorsed by, or
connected to Pulse Techniques, LLC; Pultec® and EQP-1A are their marks, used
here only to say which circuit is modelled.

![the plugin](doc/panel.png)

## What it models

The original is a passive LC/RC equalizer followed by a tube make-up amplifier
that undoes the network's ~22 dB insertion loss. The passive network is
simulated as a circuit rather than approximated with a bank of shelving
filters, which matters because the boost and attenuate controls of each band
tap the same divider chain at different points. Using both at once is not two
filters added together, it is one network rearranged, and that is where the
"low end trick" comes from: boosting and attenuating the same low frequency
lifts the bottom and digs a dip an octave or two above it. The original manual
told you not to do it.

* **Topology and values** are taken from the original schematic: a 10k high boost
  pot with the LC tank bridged off its wiper, a 1k high attenuate pot shunting
  the top end to ground through the selected capacitor, a 100k low attenuate
  pot and a 10k low boost pot with their own pairs of frequency capacitors.
  Pot tapers follow the Pultec service documentation, so the low boost and low
  attenuate knobs are audio taper and the rest are linear. The knobs are
  calibrated 0 to 10 like the hardware, and the non-linear relationship between
  knob position and decibels falls out of the circuit rather than being dialled
  in by hand.
* **Solved per sample** by nodal analysis with trapezoidal companion models,
  the way SPICE solves a linear circuit. The conductance matrix only changes
  when a knob moves, so it is factorised at control rate and each sample costs
  one forward and one back substitution.
* **Frequency prewarped** per element, so the 16 kHz bell sits at 16 kHz even at
  a 44.1 kHz sample rate instead of sliding down towards Nyquist.
* **The make-up amplifier** contributes the rest of the character: mostly odd
  order harmonics from the push-pull stage with a little second order from its
  imbalance, plus the bandwidth limits of the iron at both ends. It is clean
  at the usual −18 dBFS operating level and runs gently out of headroom as the
  peaks approach full scale. The `DRIVE` control is the one liberty taken with
  the hardware: it hits the amplifier harder, as more level into the hardware
  would, so the sound gets louder and dirtier together.

Measured against the published curves, with the tests in `tests/response.rs`
checking each one:

| Setting | Response |
| --- | --- |
| Low boost 10 @ 100 cps | +16.3 dB at 20 Hz |
| Low atten 10 @ 100 cps | −19.2 dB at 20 Hz |
| Low boost 10 + atten 10 @ 30 cps | +5.1 dB at 80 Hz, −4.2 dB at 200 Hz |
| High boost 10, sharp | +18.0 dB on the selected frequency |
| High boost 10, broad | +12.9 dB, wider |
| High atten 10 | −16 dB on the selected frequency |

## Controls

The front panel is the hardware's. The left hand knob is the EQ IN/OUT
switch, which lifts the passive network out of circuit but leaves the amplifier
in it. The OFF/ON knob at the right takes the whole unit out of circuit, and
the pilot lamp follows it. Switched off, the signal still comes out as late as
the reported latency says, so the track stays in time with the rest of the
session whichever way the switch sits.

The strip above the panel is not on the hardware. It carries the preset drop
down, a save button and the settings button.

Nor are the two level meters, one either side of the controls, there for gain
staging: **INPUT** on the left reads what arrives, and **OUTPUT** on the right
what leaves, after the output trim, or the dry signal with the power off. They
read in dBFS, on the scale DAW peak meters use, from −60 to +6, with a bar per
channel. Each bar rises to a peak at once and falls back at 20 dB in 1.7 s,
with a line holding its highest point for two seconds; it is solid up to the
RMS level and fainter from there to the peak. A lamp above each bar lights at
the first sample at or over full scale and stays lit. Under each meter are the
figures a level is set by, to a tenth of a decibel: **PEAK**, the highest
sample since it was cleared, which turns red once full scale has been reached,
and **RMS**, the average over the last 300 ms, which a full scale sine reads as
−3.0 and which moves five times a second so it can be read. Clicking a meter or its peak figure clears them. They are sample peaks;
a peak between samples is not measured.

Right of the output meter, where they can be set against it, are the
amplifier's two trims, also not on the hardware: **DRIVE** above, up to 18 dB
more level into the amplifier, and **OUTPUT** below, ±24 dB, each with its
setting lettered under it. Turning DRIVE up makes the sound louder and
dirtier, as it would on the hardware; bring the level back down with OUTPUT,
watching the output meter.

Up to 0.11, DRIVE was a saturation amount from 0 to 100 % that kept the level
and squashed only the peaks, so turning it up made loud material quieter. A
session saved with it opens with DRIVE at 0 dB, and Low End Punch has been
moved across to settings that sound as it did.

Presets are the panel's parameter values, minus the oversampling setting,
which is a choice about the machine rather than the sound. The loaded preset's
name is saved with the session, and an amber dot appears next to it as soon as
the panel no longer matches what was loaded. That is decided by comparing the
values rather than by tracking an edited flag, so turning a control back to
where it was clears the dot again. **Low End Punch** is
built in: the low end trick at 100 cps with a little 10 kc air, which measures
+7.7 dB at the bottom, +6.4 dB at 100 Hz, a scoop through 500 Hz and +6 dB of
air on top. Saving asks for a name, and confirms first if that name is already
one of yours, whatever its case, and then replaces that preset in whichever
file it lives. A new name that would come out as an existing file's name, as
"A/B" and "A_B" both would, gets a numbered file of its own rather than taking
the other one over. Saved presets are one JSON file each, under
`~/.config/pulteqfx/presets` on Linux and macOS and
`%APPDATA%\PultEQFx\Presets` on Windows, so they can be copied between machines
or edited by hand, and each carries a cross to delete it, which asks before
removing the file.

A built-in preset has no file, so it cannot be deleted, and saving under its
name writes a preset of your own beside it rather than replacing it in the
list. Replacing it would put it out of reach for good: there would be no file
left to delete to get it back.

The settings button holds:

* **Window size**, from 50 % to 200 %. The faceplate, lettering and scales are
  drawn rather than pictured, so they stay sharp at any size, and the rendered
  metal is generated large enough to hold up at the top of the range.
* **Oversampling**, off through 8x. The equalizer is prewarped and accurate
  without it, and so is the amplifier's frequency response, which stays within
  a tenth of a decibel of the analog circuit at every setting; oversampling is
  there for the amplifier's saturation. The plugin always reports 74 samples of
  latency and pads the shorter settings out to match, so switching quality
  never changes the reported latency while the host is running. A new setting
  is brought up alongside the old one and faded in a fifth of a second later,
  rather than dropping out while it fills.

Knobs respond to drag, scroll, shift for a finer grip and double click to
reset. The switches turn by drag or scroll, or by clicking one of the values
engraved round them, and the two way ones, EQ IN/OUT and OFF/ON, also throw
with a click on the knob.

## The panel

The faceplate, the lettering, the dial scales and every pointer are drawn with
vector paths, so they stay sharp at any window size. Everything that is metal
is a render: the five large knobs, the five knurled switch knobs, the pilot
lamp and the four mounting screws.

The renders come from `assetgen`, a renderer in this repository. It takes
either parametric geometry or a glTF model, bakes occlusion into it by casting
rays against itself, and shades it under a lighting rig the whole panel shares.
The large knobs are built from geometry; the switch knobs come from
`assets/knob_metal.glb` and the lamp from `assets/led_ring.glb`. glTF states
base colour, metalness and roughness but has no way to state a finish, so the
brushing and micro-texture of the aluminium are assetgen's.

Nothing on the panel is ever drawn by rotating a picture of itself. Rotating
the picture rotates the light baked into it, so the highlight would travel
round with the part instead of staying where the panel lamp is. The three
consequences are worth naming, because each one is why a part is made the way
it is:

* **The large knobs** are filmstrips — 48 frames across the 250 degree sweep,
  each lit from the same place, and the widget picks the frame matching its
  value.
* **The switch knobs** are a single frame. The knurled cylinder has no
  indicator on it and looks the same at every angle, so the widget draws the
  pointer on top at whatever angle the switch is thrown to.
* **The screws** are four separate renders, each already driven to its own
  angle, because a screw sits where it was tightened.

**The pilot lamp is rendered twice**, once with the lamp behind the jewel
burning and once with it out, rather than dimming one render in the widget. A
dark jewel is not a bright one with less light on it: its specular stays
exactly where it was while the body of it goes out, and that is what tells the
eye the lamp is off rather than the room.

### One light

The panel is lit from a single lamp, hung a little above the top edge and a
little in from the left. Four separate pieces of code have to agree about it:
the faceplate paints its highlight there, assetgen keys every render from up
and to the left, each control casts a contact shadow down and to the right, and
each control is tinted by how far it is bolted from the lamp.

That last one is the part a render cannot carry. A render knows the *direction*
its light came from, but nothing about the panel it ends up on — so without the
tint a knob at the far end of a nineteen inch panel is as bright as one under
the lamp, which is the giveaway that a panel is a collage rather than a
photograph. The falloff is linear in distance rather than inverse square,
because a faceplate is lit by a broad source at a distance, and an inverse
square across this width puts the right hand end in the dark.

`tests/lighting.rs` holds the four pieces to it: that shadows fall away from
the light, that the panel's brightest point is under the lamp and its darkest
is the opposite corner with no rise in between, that the corner-to-corner
falloff is visible without being theatrical, and that no control is tinted
brighter than the render it was made from.

Regenerate the renders with:

```sh
./assetgen/render.sh
```

The renderer is deterministic, so re-running it without changing `assetgen`
reproduces the same files byte for byte.

## Building

Needs a Rust toolchain and the usual X11 development packages.

```sh
./install.sh
```

That builds the plugin and installs the CLAP and VST3 into
`~/.clap/BurningTreeC` and `~/.vst3/BurningTreeC`. Pass `--no-build` to install
what is already built, or set `CLAP_PATH` and `VST3_PATH` to install somewhere
else.

To build without installing:

```sh
cargo xtask bundle pulteqfx --release
```

This writes `PultEQFx.clap` and `PultEQFx.vst3` to `target/bundled`.

To try it without a host:

```sh
cargo run --release --features standalone -- --backend auto
```

## Licensing

PultEQFx is under the **GNU General Public License version 3 or later**, whose
text is in [`LICENSE`](LICENSE).

That is not a free choice. NIH-plug itself is ISC licensed, but
`nih_export_vst3!()` links the [vst3-sys](https://github.com/RustAudio/vst3-sys)
bindings, which are GPLv3, so any VST3 built with NIH-plug has to be able to
comply with the GPL. Dropping the VST3 export and shipping only the CLAP would
free the plugin to use a permissive licence instead; every other crate it links
is permissive.

Three dependencies are worth naming directly:

* **NIH-plug** and its companion crates are under the
  [ISC licence](https://www.isc.org/licenses/), copyright Robbert van der Helm.
* **vst3-sys** is GPLv3, which is what makes the plugin as a whole GPL.
* **Noto Sans** is compiled into the binary for the panel lettering. The fonts
  come from `nih_plug_assets`, which is itself ISC, but the font files are
  under the SIL Open Font License 1.1, copyright The Noto Project Authors. That
  licence requires it travel with the binary, so it is reproduced in full.

[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md) reproduces the licences and
copyright notices of all 286 crates PultEQFx links, grouped by licence. Where a
crate offers a choice, the licence taken is named, and where a crate bundles
assets under a different licence than its own, that is called out too.
Regenerate it after changing dependencies:

```sh
python3 tools/third-party-notices.py
```

## Layout

| Path | |
| --- | --- |
| `src/dsp/eqp1a.rs` | the passive network, its component values and pot tapers |
| `src/dsp/nodal.rs` | the nodal solver and the companion models |
| `src/dsp/tube.rs` | the make-up amplifier |
| `src/dsp/oversample.rs` | linear phase halfband oversampling |
| `src/editor/` | the front panel |
| `src/editor/style.rs` | the drawn controls and the panel's colours |
| `src/editor/sprites.rs` | the rendered knob and the screws |
| `assetgen/` | the renderer that generates the knobs |
| `src/presets.rs` | built-in and saved presets |
| `src/meters.rs` | the input and output levels the meters read |
| `src/editor/meter.rs` | the meters themselves |
| `tests/response.rs` | frequency response against the published curves |
| `tests/latency.rs` | the reported latency, and switching without a gap |
| `tests/drive.rs` | what DRIVE does, and Low End Punch against the old drive |
| `tests/presets.rs` | preset storage round trip |
| `tests/state.rs` | what survives a save and reload |
| `tools/third-party-notices.py` | regenerates the dependency licence file |
