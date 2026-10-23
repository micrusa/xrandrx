# xrandrx

A vibe-coded lightweight, zero-dependency CLI extension for `xrandr` to manage multi-monitor setups effortlessly.

## Build & Install

```bash
# Build binary
./build.sh

# Install to ~/.local/bin (or specify custom path: ./install.sh /custom/bin)
./install.sh
```

## Usage

```bash
# Arrange screens left-to-right (default: best quality, center screen primary)
xrandrx --ltr hdmi edp dp

# Arrange screens right-to-left
xrandrx --rtl dp edp hdmi

# Mirror screens (clone mode)
xrandrx --mirror hdmi edp

# Override primary display
xrandrx --ltr hdmi edp dp --primary edp

# Preview generated xrandr command without applying changes
xrandrx --dry-run --ltr hdmi edp dp

# List detected screens and available modes
xrandrx --list
```

> **Note:** `xrandrx --ltr hdmi edp dp` is equivalent to `xrandrx --ltr hdmi edp dp --primary edp --best`.

## Monitor Aliases

- Exact output names work: `HDMI-0-1`, `eDP-1`, `DP-1`.
- If only one port of a connector type exists, use its name directly: `hdmi`, `edp`, `dp`.
- For multiple ports of the same type, enumerate from 0:
  - `hdmi` or `hdmi0` selects the 1st HDMI output.
  - `hdmi1` selects the 2nd HDMI output.

## Options

| Option | Description |
|---|---|
| `--ltr` | Align screens Left-to-Right (default). |
| `--rtl` | Align screens Right-to-Left. |
| `--mirror`, `-m` | Mirror screens (clone mode). |
| `--best` | Use highest resolution and highest refresh rate for that resolution (default: on). |
| `--primary <SCREEN>` | Set primary screen (default: center screen, or highest resolution between the two center screens if even). |
| `--dry-run`, `-n` | Print the `xrandr` command without executing it. |
| `--list`, `-l` | Show detected monitors and resolutions. |
| `--keep-others` | Keep unselected connected displays on (default turns them off). |
| `-h`, `--help` | Show help. |
