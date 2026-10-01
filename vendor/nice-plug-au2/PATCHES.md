# Local nice-plug-au2 changes

Upstream: fazibear/nice-plug-addons, `crates/nice-plug-au2` (manifest 0.1.1),
<https://codeberg.org/fazibear/nice-plug-addons>. Vendored from GainStageFx's
copy, which carries the changes below; keep the two in step.

- Depend on nice-plug-core at the plugin's pinned git revision (0.4.2)
  rather than crates.io 0.1.1, so one copy of the framework is linked, and
  drop the unused `nice-plug-derive`.
- Compile the Objective-C Cocoa view factory (`src/bridge/shim.m`) in
  `build.rs`. AUv2 hosts find the editor through Objective-C class metadata,
  so `NiceAu2CocoaViewFactory` has to exist in the final Mach-O image.
- `editor_host` returns no host methods, which builds on macOS.
- Answer `kAudioUnitProperty_ClassInfo` in the global scope, for both reading
  and writing. The dictionary carries the plugin's own state blob
  (`nice-plug-state`) and the identity fields auval checks: `type`,
  `subtype`, `manufacturer`, `version` and `name`. Those are read from the
  `Au2Plugin` the plugin exports (`factory::plugin_config`) rather than
  written in, so the bridge serves any plugin that vendors it. GainStageFx's
  copy still has its own codes written in here.
- Refuse a stream format whose channel count differs from the bus layout the
  unit advertises, so `SupportedNumChannels` and the formats cannot disagree.
- Formatted with rustfmt, as a member of the plugin's workspace.
- `LICENSE`: upstream names ISC in its manifest but ships no licence text.
  This is the ISC text with the author from the manifest, which the
  distribution has to carry.
