# Gander

A terminal markdown viewer with image support.

## Features

- Rich markdown rendering with syntax-highlighted code blocks
- Image support via Kitty, Sixel, and half-block fallback
- Table of contents sidebar
- File watching for live preview
- Vim-style navigation

## Installation

```bash
cargo install --path .
```

## Usage

```bash
gander README.md
gander --watch README.md    # Auto-reload on file changes
gander --toc README.md      # Start with TOC visible
gander --force-half-cell README.md  # Force half-cell image rendering (debug)
```

## Key Bindings

| Key | Action |
|-----|--------|
| `j`/`k` | Scroll down/up |
| `Space`/`b` | Page down/up |
| `g`/`G` | Go to top/bottom |
| `t` | Toggle TOC sidebar |
| `w` | Toggle file watching |
| `/` | Search |
| `q` | Quit |

## License

MIT
