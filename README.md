# repose-audui

Tested with a clap plugin (ednitar-clap), and works well. Limitations are mostly due to this and not repose (was made to check if it'll work, and archiving it since it only exists to demonstrate a simple plugin. 
You might need to fork it for advanced features.)

Embed [Repose](https://github.com/mlm-games/repose) UI in DAW audio plugins via
**baseview** / **nih-plug**. This is the audio-plugin (like iced-baseview) mirror of
[repose-bevy](https://github.com/mlm-games/repose-bevy) and intentionally lives
**outside** `repose-platform`. (UI thread only)

## Features

| Feature     | Purpose                                                   |
|-------------|-----------------------------------------------------------|
| (none)      | `ReposeEmbed`, wgpu attach, input helpers                  |
| `baseview`  | Parented / blocking baseview window host (+ clipboard)     |
| `nih-plug`  | `create_repose_editor` for nih-plug plugins (implies baseview) |

## Quick test

```bash
cargo run --example gain_gui --features baseview
```

## nih-plug

```toml
repose-audui = { git = "https://github.com/mlm-games/repose-audui", features = ["nih-plug"] }
```

The `baseview` dependency is pinned to the exact Git revision that
`nih-plug` master pins, so one plugin graph ends up with a single `baseview`
and a single `raw-window-handle` 0.5 (wgpu 30 uses rwh 0.6; the bridge lives in
`src/rwh_compat.rs`).

## Notes

- All runtime input stays in **physical pixels**; the scale set on
  `set_size_px` matches baseview's window scale factor. Doesn't resize at the moment.

## License

MPL-2.0 OR LGPL-2.1-or-later.
