PultEQFx
========

The easy way
------------
Double-click Install.command. It copies the CLAP, VST3 and Audio Unit plugins
into place and clears the macOS quarantine flag described below.

The first time, macOS will refuse to run it because it came from the internet.
Right-click Install.command, choose Open, then confirm in the dialog. After
that it runs normally.

By hand
-------
Copy the bundles (they are folders, copy the whole thing) to:

  CLAP   ~/Library/Audio/Plug-Ins/CLAP/PultEQFx.clap
  VST3   ~/Library/Audio/Plug-Ins/VST3/PultEQFx.vst3
  AU     ~/Library/Audio/Plug-Ins/Components/PultEQFx.component

(Install.command puts the CLAP and VST3 in a BurningTreeC subfolder of each,
which hosts search just the same. The Audio Unit has to go in Components
itself.)

Then open Terminal and run:

  xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/CLAP/PultEQFx.clap
  xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/PultEQFx.vst3
  xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/Components/PultEQFx.component
  killall -9 AudioComponentRegistrar

The last line makes macOS forget what it knew about Audio Units, so a host
sees the new one; it is restarted on its own.

Why that last step is needed
----------------------------
The plugins are signed, but only ad-hoc: signed by nobody in particular. That
is enough for macOS to load them, and on Apple Silicon it is required, but it
is not the same as being notarized by Apple. Notarizing needs a paid Apple
Developer account, which this project does not have.

macOS tags anything downloaded from the internet with a quarantine flag, and
refuses to load quarantined code that is not notarized. Removing the flag with
the command above tells macOS you trust these files. Nothing else about the
plugins changes.

Logic and GarageBand load only Audio Units, which is what the .component is
for. It is an Audio Unit version 2 effect, listed under BurningTreeC.

This program comes with ABSOLUTELY NO WARRANTY. It is free software under the
GNU General Public License version 3 or later; see LICENSE. The licences of
the libraries it uses are in THIRD-PARTY-NOTICES.md.
Source: https://github.com/BurningTreeC/pulteqfx
