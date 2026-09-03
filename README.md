# rift-container-highlighter

Flashes an outline around every container in the active [rift](https://github.com/acsandmann/rift)
workspace, so you can see the container tree before running a command that acts on it.

In rift's `traditional` and `bsp` layouts the tree is invisible. Two windows side by side look
identical whether they are siblings in the root container or a nested sub-container next to a third
window — which makes `move_node`, `join_window`, `unjoin_windows`, `toggle_orientation` and
`ascend`/`descend` guesswork. This draws the tree for a second when you ask, or right after you
change it.

Each container gets a band that is opaque at its outer edge and fades to transparent inward across
the member windows. Colour comes from nesting depth, and deeper containers get narrower bands so a
child sharing an edge with its parent still reads as two levels.

The container drawn at **full opacity is the one holding the layout selection — what the next
structural command will act on.** That is the useful part. `ascend` and `descend` do not move
anything; they walk the selection up and down the tree, so that `move_node` moves a single window or
a whole container depending on where the selection sits. Without a visual cue you find out by
pressing the key and undoing it. If the selection is on a bare window, no container is bright; if it
has been walked all the way up, the whole workspace is outlined.

## Why outlines and not coloured window borders

Recolouring each window's border (via JankyBorders' `apply-to`, say) is much less work and needs no
drawing at all. It cannot do the job. Any per-window property — border colour, background,
brightness, alpha — gives each window exactly one value, which is a *partition*. Nesting is a
hierarchy, so no assignment of per-window colours can show that container B sits inside container A
while both are on screen. Showing every container simultaneously requires drawing, so this draws.

## Requirements

- macOS. Uses SkyLight, a private framework.
- rift, running. Version-sensitive: see [Versioning](#versioning).
- Accessibility permission is needed by **rift**, not by this tool. It only reads rift's IPC and
  creates its own overlay window.

## Install

```sh
cargo install --path .
```

## Usage

```
rift-container-highlighter peek                    # flash the current tree
rift-container-highlighter peek --ms 10000         # ... and hold it, for tuning
rift-container-highlighter wrap ascend             # run a rift command, then flash
rift-container-highlighter wrap join-window left
rift-container-highlighter reset                   # clear a flash left on screen
rift-container-highlighter themes                  # theme names with a built-in palette
rift-container-highlighter dump                    # layout, frames, computed rects, effective config
```

`wrap` accepts the structural commands only — `ascend`, `descend`, `move-node`, `join-window`,
`consume-or-expel-window`, `toggle-stack`, `toggle-orientation`, `unjoin` — the ones whose effect on
the tree you cannot otherwise see. It runs the rift command *first*, so keypress latency does not
depend on the flash.

There is no daemon. Each invocation queries rift, draws, holds, and exits; the window server
discards the overlay when the process ends, which is also why `reset` is just a kill.

A flash retires any flash already on screen before drawing, so tapping a binding repeatedly does
not leave a stale overlay showing the previous tree for the rest of its own timer.

## Keybindings

Three bindings are enough. Nothing needs to be rebound:

```toml
# ~/.config/rift/config.toml
"Alt + Backslash"    = { exec = "/path/to/rift-container-highlighter peek" }
"Alt + BracketLeft"  = { exec = "/path/to/rift-container-highlighter wrap ascend" }
"Alt + BracketRight" = { exec = "/path/to/rift-container-highlighter wrap descend" }
```

None of the three modifies the tree, so they belong with your focus bindings rather than with the
ones that move windows.

Use an **absolute path**: rift runs as a launchd agent with no `PATH`, so a bare binary name
silently does nothing.

Punctuation keys are worth preferring here. A rift hotkey swallows the key before the terminal sees
it, so a letter is easy to lose to something else: `Alt + P` is Claude Code's model switcher and
`Alt + D` is zsh's `kill-word`.

`ascend` and `descend` are wrapped because they are the only structural commands that emit no
event — nothing outside the keybinding can notice a selection change. Everything else
(`join_window`, `move_node`, `toggle_orientation`, `unjoin`) already fires `layout_changed`, so if
you want those to flash automatically, subscribe instead of rebinding:

```sh
rift-cli subscribe cli --event layout_changed \
  --command /path/to/rift-container-highlighter --args peek
```

That fires on divider drags and on windows opening and closing too, so it flashes more than most
people want. Suppressing that needs a structure hash cached between runs, which this does not do.

rift keybindings fire on key-down only — there is no release action — so hold-to-peek is not
possible. `peek` is a timed flash.

## Configuration

Optional, at `~/.config/rift-container-highlighter/config.toml`. See
[`config.example.toml`](config.example.toml) for every key and its default. The file is re-read on
every invocation, so changes apply to the next flash with nothing to reload.

`corner_radius` is the one worth tuning: macOS 26 rounds window corners considerably more than
earlier releases, and this tool does not read the real per-window value. JankyBorders does, via
`SLSWindowIteratorGetCornerRadii`, which would mean vendoring the window-iterator API.

Red is deliberately absent from every built-in palette. Add it to `palette` if you want it — the
reasoning is in `config.example.toml`.

### Theme integration

`theme` takes a canonical name and selects a built-in palette. If you drive your tools from a single
script, have it write that one line; `rift-container-highlighter themes` lists the names that have a
palette, so the script can warn instead of letting an unknown name fall back.

## Versioning

`rift-client` and `rift-protocol` are git dependencies pinned to the tag of the rift release this
was built against — currently `v0.5.5`. **Pin the tag of the rift you actually run.** The IPC wire
format drifts between releases: built against `main`, `get_layout_state` fails against a 0.5.5
daemon with `data did not match any variant of untagged enum RiftResponse`.

**Do not `cargo add rift-client` or `rift-protocol`.** Those crates.io names belong to unrelated
third-party projects; rift publishes neither.

On a rift upgrade: bump the rev in `Cargo.toml`, re-diff `src/vendor/` against the new tag, rebuild,
and run `dump` before anything else.

## Vendored code

`src/vendor/` contains rift's CGS overlay bindings, copied under Apache-2.0 — see
[`NOTICE`](NOTICE). This is the upstream-endorsed arrangement rather than a stopgap:
[rift#467](https://github.com/acsandmann/rift/issues/467) asked for them as a published crate and
was closed, since `rift-client` is IPC-only by design. Deviations from upstream are listed in each
file's header and are mechanical only.

One thing that cost real time and is worth knowing if you write something similar: **a CFRunLoop
must be turning or the overlay is never composited**. Create the windows and then block in
`thread::sleep` and you get an empty screen with every CGS call returning success and no error
anywhere.

## Known limits

- **Container rects are approximate.** A container's rect is the union of its member windows'
  frames, grown by half the inner gap, because the protocol exposes no per-node geometry.
  [rift#466](https://github.com/acsandmann/rift/issues/466) asks for it.
- **Colours are keyed to depth, not container identity**, so a container does not keep its colour
  across a structural change. Tree nodes have no stable id, deliberately — upstream documents that
  internal ids are not stable across mutations and suggests identifying nodes by path.
- **`ascend`/`descend` emit no event**, so nothing outside the `wrap` keybindings can notice a
  selection change. Also in [rift#466](https://github.com/acsandmann/rift/issues/466).
- Nesting deeper than the palette wraps colours.

## Licence

Apache-2.0. See [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE).
