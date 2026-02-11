# Markless

Markless is a terminal markdown viewer and editor with image support. It is focused on fast navigation, clear rendering, and sensible defaults for long documents.

## Screenshots

![Table of Contents sidebar](<images/table of contents.png>)
![Directory browse mode](<images/browser.png>)
![Inline image support using Kitty](<images/image support.png>)
![Code syntax highlighting](<images/code highlighting.png>)
![Table Support](images/tables.png)
![Editor mode](images/editmode.png)

## Features

- Markdown rendering with headings, lists, tables, block quotes, code blocks, and footnotes
- Syntax-highlighted code blocks with lazy highlighting for performance
- Inline images (Kitty, Sixel, iTerm2, and half-block fallback)
- Built-in editor mode with save, undo-friendly exit, and conflict detection
- Directory browse mode with file preview
- Table of contents sidebar with keyboard and mouse support
- Search with match navigation and highlight
- File watching for live reload
- Link hover and click (including image placeholders)
- Line selection with mouse drag and copy
- Fast scrolling with stable layout and reflow on resize

## Installation

### Development (from source)

```bash
cargo install --path .
```

### Production (from crates.io)

```bash
cargo install markless
```

## Usage

```bash
markless README.md          # View a file
markless .                  # Browse current directory
markless                    # Browse current directory (default)
markless src/               # Browse a directory
```

When given a directory, markless opens in browse mode: the sidebar shows the file listing and the first markdown file (or first file) is previewed automatically. Navigate with arrow keys, press Enter to open, and Backspace to go to the parent directory.

## Command Line Options

- `--watch`  Auto-reload on file changes
- `--no-toc`  Hide the table of contents sidebar
- `--toc`  Start with TOC visible
- `--no-images`  Disable inline image rendering (show placeholders only)
- `--image-mode <kitty|sixel|iterm2|halfblock>`  Force a specific image rendering protocol
- `--force-half-cell`  Force half-cell image rendering (shortcut for `--image-mode halfblock`)
- `--theme <auto|light|dark>`  Force highlight theme background
- `--perf`  Enable startup performance logging
- `--render-debug-log <PATH>`  Write render/image debug events to a file
- `--save`  Save current flags as defaults in the global config
- `--clear`  Clear saved defaults in the global config

Config files:
- Global (macOS): `~/Library/Application Support/markless/config`
- Local override: `.marklessrc` in the current directory

## Key Bindings

Navigation
- `j` / `k` or arrows: scroll
- `Space` / `PageDown`: page down
- `b` / `PageUp`: page up
- `Ctrl-d` / `Ctrl-u`: half page
- `g` / `G`: top / bottom

Search
- `/`: start search
- `Enter`: next match
- `Esc`: clear search

TOC
- `t`: toggle TOC
- `T`: toggle + focus TOC
- `Tab`: switch focus
- `j` / `k`, arrows, `Enter` / `Space`: navigate + jump
- `h` / `Left`: collapse (parent directory in browse mode)
- `l` / `Right`: expand (enter directory in browse mode)

Browse
- `B`: enter directory browse mode
- `F`: return to file-only mode (heading TOC)
- `Backspace`: navigate to parent directory (in TOC)

Editor
- `e`: enter edit mode
- `Ctrl-e`: toggle edit mode (enter or exit)
- `Esc`: return to view mode
- `Ctrl-s`: save file
- Arrows, Home/End: navigate
- `Ctrl+Left` / `Ctrl+Right`: word movement
- `Ctrl+Home` / `Ctrl+End`: buffer start / end
- PageUp/PageDown: scroll editor

Other
- `w`: toggle watch
- `r` / `R`: reload file
- `o`: open visible links (1-9)
- `?` / `F1`: toggle help
- `q` / `Ctrl-c` / `Ctrl-q`: quit

Mouse
- Scroll wheel: scroll
- Click links or images: open
- Hover link/image: show URL
- Click + drag: select lines and copy

## License

MIT
