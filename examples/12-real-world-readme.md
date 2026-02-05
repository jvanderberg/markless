# Markless - Terminal Markdown Viewer

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)]()
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)]()

A fast, feature-rich terminal-based markdown viewer with image support.

## Features

- **Full CommonMark Support** - Headings, lists, code blocks, and more
- **GFM Extensions** - Tables, task lists, strikethrough, autolinks
- **Image Rendering** - Kitty, Sixel, iTerm2, and halfblock protocols
- **Syntax Highlighting** - 100+ languages via syntect
- **Table of Contents** - Navigate large documents easily
- **Live Reload** - Watch files for changes
- **Keyboard Navigation** - Vim-like keybindings

## Installation

### From Cargo

```bash
cargo install markless
```

### From Source

```bash
git clone https://github.com/example/markless
cd markless
cargo build --release
```

## Quick Start

```bash
# View a markdown file
markless README.md

# Enable file watching
markless --watch document.md

# Show table of contents
markless --toc notes.md
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `d` | Half page down |
| `u` | Half page up |
| `g` | Go to top |
| `G` | Go to bottom |
| `t` | Toggle TOC |
| `/` | Search |
| `n` | Next match |
| `N` | Previous match |
| `q` | Quit |

## Configuration

Create `~/.config/markless/config.toml`:

```toml
[display]
theme = "dracula"
show_line_numbers = true
wrap_mode = "word"

[keys]
scroll_amount = 3
```

## Requirements

- Rust 1.75 or later
- A terminal with:
  - [x] 256 color support
  - [x] Unicode support
  - [ ] Image protocol support (optional)

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing`)
3. Make your changes
4. Run tests (`cargo test`)
5. Submit a pull request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

Made with ❤️ by the Markless team
