# cosmic-ext-applet-executor

A COSMIC desktop applet that runs shell commands and displays their output in the system panel.

Inspired by [Executor](https://extensions.gnome.org/extension/2932/executor/) for GNOME Shell by [raujonas](https://github.com/raujonas/executor).

The popup layout is also inspired by the Executor GNOME extension UI.

Popup open/close handling is based on patterns from [cosmic-applets](https://github.com/pop-os/cosmic-applets) (cosmic-applet-time), © System76, licensed under GPL-3.0.

## Features

- Multiple independent blocks, each with its own shell command and refresh interval
- ANSI color codes rendered natively (`\e[31m` etc.)
- Hot-reload config — no restart needed
- Non-blocking execution (UI stays responsive)
- Configurable separator between blocks
- Configurable font size

## Installation

```sh
sudo apt install libxkbcommon-dev just
git clone https://github.com/ririck0/cosmic-ext-applet-executor.git
cd cosmic-ext-applet-executor
cargo build --release
sudo just install
```

Then in COSMIC: right-click panel → **Edit Panel** → **Add Applet** → search "executor".

## Usage

Click the applet in the panel to open the settings popup.

### Blocks

Each block runs an independent shell command at its own interval and displays the output in the panel.

| Control | Action |
|---|---|
| `+` (bottom left) | Add a new block |
| `🗑` | Remove a block |
| `↑` / `↓` | Reorder blocks |
| `−` / `+` (next to interval) | Decrease / increase refresh interval |

- **Command** — any shell command, executed via `sh -c`
- **Interval** — refresh interval in seconds (minimum 1)

### Display settings

- **Separator** — text shown between blocks in the panel
- **Font size** — use `−` / `+` to adjust

Click **Save** to persist the configuration.

## Configuration file

Settings are stored at:

```
~/.config/cosmic/io.github.cosmic_utils.cosmic-ext-applet-executor/v1/config.json
```

Example:

```json
{
  "separator": " | ",
  "font_size": 14.0,
  "blocks": [
    { "command": "date +%H:%M:%S", "interval": 1 },
    { "command": "sensors | grep Package | head -c23 | cut -d' ' -f5", "interval": 3 },
    { "command": "ip -br a | grep eth0 | awk '{print $3}'", "interval": 10 }
  ]
}
```

| Field | Type | Default | Description |
|---|---|---|---|
| `blocks` | array | `[]` | List of blocks |
| `blocks[].command` | string | — | Shell command (`sh -c`) |
| `blocks[].interval` | integer | `5` | Refresh interval in seconds |
| `separator` | string | `" \| "` | Text between blocks in the panel |
| `font_size` | float | theme default | Font size in points |

## ANSI colors

Scripts can emit ANSI color codes — the applet renders them as colored text.

```bash
echo -e "\e[32mOK\e[0m"
echo -e "\e[31mERROR\e[0m"
echo -e "\e[33mWARN\e[0m"
```

Supported codes: 30–37 (standard) and 90–97 (bright). Reset: `\e[0m`.

## Uninstall

```sh
sudo just uninstall
```

## License

GPL-3.0 — see [LICENSE](LICENSE).

Copyright © 2026 Ririck0
