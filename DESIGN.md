# Gander - Design Document

A Rust-based TUI markdown viewer with image support.

## Table of Contents

1. [Overview](#overview)
2. [Goals & Non-Goals](#goals--non-goals)
3. [Architecture](#architecture)
4. [Core Components](#core-components)
5. [Viewing Modes](#viewing-modes)
6. [Table of Contents Sidebar](#table-of-contents-sidebar)
7. [File Watching](#file-watching)
8. [Markdown Processing](#markdown-processing)
9. [Image Rendering](#image-rendering)
10. [Syntax Highlighting](#syntax-highlighting)
11. [Key Bindings](#key-bindings)
12. [Project Structure](#project-structure)
13. [Dependencies](#dependencies)
14. [Testing Strategy](#testing-strategy)
15. [Development Tooling](#development-tooling)
16. [Future Considerations](#future-considerations)

---

## Overview

**Gander** is a terminal-based markdown viewer that combines the best aspects of pager-style navigation (like `less`) with full-screen scrollable viewing (like an editor). It renders markdown with rich formatting and supports inline images via Kitty graphics protocol, Sixel, and Unicode half-block fallback.

### Why "Gander"?

"Take a gander" means to take a look - fitting for a document viewer. Short, memorable, and available in the Rust ecosystem.

---

## Goals & Non-Goals

### Goals

- **Fast startup** - Open and render large markdown files quickly
- **Rich rendering** - Full markdown formatting with syntax-highlighted code blocks
- **Image support** - Native terminal image rendering with graceful fallback
- **Intuitive navigation** - Familiar keybindings from `less`, `vim`, and modern editors
- **Unified viewing mode** - Seamless blend of pager and editor-style navigation
- **Table of contents** - Toggleable sidebar for document structure navigation
- **Live reload** - Auto-refresh on file changes for previewing while editing
- **Standards compliant** - CommonMark + widely-used GFM extensions
- **Testable** - Comprehensive test coverage via TDD
- **Well-documented** - Full rustdoc coverage

### Non-Goals

- **Editing** - This is a viewer, not an editor
- **Format conversion** - No export to HTML/PDF (use pandoc)
- **Custom markdown extensions** - Stick to established standards
- **GUI** - Terminal-only

---

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI Interface                           │
│                        (clap argument parsing)                  │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Application                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   State     │  │   Event     │  │       Renderer          │ │
│  │  Manager    │◄─┤   Handler   │  │  (ratatui + crossterm)  │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
│         ▲                                                       │
│         │         ┌─────────────┐                               │
│         └─────────┤    File     │ (notify crate)                │
│                   │   Watcher   │                               │
│                   └─────────────┘                               │
└─────────────────────────────────────────────────────────────────┘
                                  │
                    ┌─────────────┼─────────────┐
                    ▼             ▼             ▼
          ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
          │  Markdown   │ │   Image     │ │   Syntax    │
          │  Parser     │ │  Renderer   │ │ Highlighter │
          │  (comrak)   │ │(ratatui-img)│ │  (syntect)  │
          └─────────────┘ └─────────────┘ └─────────────┘
```

### Design Pattern: The Elm Architecture (TEA)

We adopt TEA for its simplicity and testability:

- **Model** - Application state (scroll position, document, viewport, mode)
- **Message** - User actions and events (scroll, search, resize, quit)
- **Update** - Pure function: `(Model, Message) -> Model`
- **View** - Pure function: `Model -> Frame` (rendered via ratatui)

This enables:
- Easy unit testing of state transitions
- Predictable behavior
- Clear separation of concerns

```rust
// Conceptual structure
struct Model {
    document: RenderedDocument,
    viewport: Viewport,
    scroll_offset: usize,
    search_state: Option<SearchState>,
    mode: ViewMode,
    toc_visible: bool,
    toc_selected: Option<usize>,
    file_path: PathBuf,
    watch_enabled: bool,
}

enum Message {
    ScrollUp(usize),
    ScrollDown(usize),
    PageUp,
    PageDown,
    GoToTop,
    GoToBottom,
    Search(String),
    NextMatch,
    PrevMatch,
    Resize(u16, u16),
    ToggleToc,
    TocSelect(usize),
    TocNavigate(Direction),
    FileChanged,
    ToggleWatch,
    Quit,
}

fn update(model: Model, msg: Message) -> Model { /* ... */ }
fn view(model: &Model, frame: &mut Frame) { /* ... */ }
```

---

## Core Components

### 1. Document Model

The parsed and rendered markdown document:

```rust
/// A fully parsed and layout-computed document
pub struct Document {
    /// Source markdown text
    source: String,
    /// Parsed AST from comrak
    ast: Arena<AstNode>,
    /// Rendered lines ready for display
    rendered_lines: Vec<RenderedLine>,
    /// Image references with positions
    images: Vec<ImageRef>,
    /// Heading positions for navigation
    headings: Vec<HeadingRef>,
    /// Link positions for potential interaction
    links: Vec<LinkRef>,
}

/// A single rendered line with styling
pub struct RenderedLine {
    spans: Vec<StyledSpan>,
    line_type: LineType,
    source_range: Option<Range<usize>>,
}

pub enum LineType {
    Paragraph,
    Heading(u8),      // level 1-6
    CodeBlock,
    BlockQuote,
    ListItem(usize),  // nesting level
    Table,
    HorizontalRule,
    Image,
    Empty,
}
```

### 2. Viewport

Manages what's visible on screen:

```rust
pub struct Viewport {
    /// Terminal dimensions
    width: u16,
    height: u16,
    /// Current scroll offset (in rendered lines)
    offset: usize,
    /// Total rendered lines in document
    total_lines: usize,
}

impl Viewport {
    pub fn visible_range(&self) -> Range<usize>;
    pub fn scroll_percentage(&self) -> f32;
    pub fn can_scroll_up(&self) -> bool;
    pub fn can_scroll_down(&self) -> bool;
}
```

### 3. Terminal Backend

Abstraction over terminal operations (enables testing):

```rust
pub trait TerminalBackend {
    fn size(&self) -> io::Result<(u16, u16)>;
    fn draw<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame);
    fn supports_kitty_graphics(&self) -> bool;
    fn supports_sixel(&self) -> bool;
}
```

---

## Viewing Modes

### Unified Scrolling Mode

Rather than separate "pager" and "editor" modes, gander uses a **unified scrolling mode** that combines the best of both:

| Feature | Pager-style | Editor-style | Gander |
|---------|-------------|--------------|--------|
| Line-by-line scroll | j/k, arrows | arrows | ✓ Both |
| Page scroll | Space, PgUp/PgDn | PgUp/PgDn | ✓ Both |
| Half-page scroll | Ctrl-d/u | - | ✓ |
| Go to top/bottom | g/G | Ctrl-Home/End | ✓ Both |
| Search | / | Ctrl-f | ✓ Both |
| Mouse scroll | - | ✓ | ✓ |
| Percentage jump | 50% | - | ✓ |
| Smooth scroll | - | ✓ | ✓ (configurable) |

### Status Line

Always-visible status line at bottom (like `less`):

```
filename.md  [73%]  Line 142/195  [Kitty]  ?:help
```

Components:
- Filename (truncated if needed)
- Scroll percentage
- Line position
- Image protocol indicator
- Help hint

### Optional Header

Configurable header showing document title (first H1) or filename.

---

## Table of Contents Sidebar

### Overview

A toggleable sidebar displaying the document's heading structure for quick navigation. Essential for long documents and documentation browsing.

### Layout

```
┌─────────────────────┬──────────────────────────────────────────┐
│ Table of Contents   │ # Document Title                         │
│                     │                                          │
│ ▸ Introduction      │ Some introductory text here that         │
│   ▸ Background      │ explains what this document is about.    │
│   ▸ Motivation      │                                          │
│ ▾ Getting Started   │ ## Getting Started                       │
│   • Installation    │                                          │
│   • Configuration   │ To get started, first install the...     │
│ ▸ API Reference     │                                          │
│ ▸ Examples          │                                          │
│                     │                                          │
├─────────────────────┼──────────────────────────────────────────┤
│ [TOC] 25%           │ README.md  [25%]  Line 42/168   ?:help   │
└─────────────────────┴──────────────────────────────────────────┘
```

### Features

| Feature | Description |
|---------|-------------|
| Toggle | `t` to show/hide sidebar |
| Navigate | `j`/`k` or arrows to move selection in TOC |
| Jump | `Enter` to jump to selected heading |
| Collapse | `h`/`l` or `←`/`→` to collapse/expand sections |
| Sync | Current heading highlighted as you scroll |
| Width | Configurable width (default: 25% or 30 chars min) |

### Data Structure

```rust
pub struct TableOfContents {
    /// All headings extracted from document
    entries: Vec<TocEntry>,
    /// Currently selected entry (for keyboard nav)
    selected: Option<usize>,
    /// Collapsed heading indices
    collapsed: HashSet<usize>,
    /// Sidebar width in columns
    width: u16,
}

pub struct TocEntry {
    /// Heading level (1-6)
    level: u8,
    /// Heading text (stripped of formatting)
    text: String,
    /// Line number in rendered document
    line: usize,
    /// Optional heading ID for anchors
    id: Option<String>,
    /// Index of parent heading (for tree structure)
    parent: Option<usize>,
}
```

### Behavior

1. **Auto-sync**: As the user scrolls, the TOC highlights the current section
2. **Bidirectional**: Selecting a TOC entry scrolls the document; scrolling updates TOC
3. **Persistence**: Remember collapsed state during session
4. **Smart width**: Adjust to content or use percentage of terminal width
5. **Focus mode**: When TOC is focused, vim-style navigation applies to TOC

### Rendering

- Indent based on heading level (2 spaces per level)
- Use tree characters: `▸` (collapsed), `▾` (expanded), `•` (leaf)
- Truncate long headings with `…`
- Highlight current section with reverse video or color
- Dim headings outside current section (optional)

---

## File Watching

### Overview

Automatic file reload when the source markdown file changes. Essential for previewing while editing in another window/pane.

### Implementation

Using the [notify](https://github.com/notify-rs/notify) crate for cross-platform file system events.

```rust
pub struct FileWatcher {
    /// The file being watched
    path: PathBuf,
    /// Debounce duration to avoid rapid reloads
    debounce: Duration,
    /// Whether watching is enabled
    enabled: bool,
    /// Channel to send reload events
    tx: Sender<WatchEvent>,
}

pub enum WatchEvent {
    Modified,
    Deleted,
    Renamed(PathBuf),
    Error(notify::Error),
}
```

### Behavior

| Event | Action |
|-------|--------|
| File modified | Reload and re-render, preserve scroll position |
| File deleted | Show warning, keep current content |
| File renamed | Follow the rename if possible |
| Rapid changes | Debounce (default: 100ms) |

### Scroll Position Preservation

When reloading, attempt to maintain the user's position:

1. **By heading**: If viewing under a heading, find same heading after reload
2. **By percentage**: Fallback to same percentage through document
3. **By line content**: Hash nearby lines, find best match

```rust
pub enum ScrollAnchor {
    /// Anchor to a specific heading by ID or text
    Heading { id: Option<String>, text: String },
    /// Anchor to percentage through document
    Percentage(f32),
    /// Anchor to content hash of nearby lines
    ContentHash { hash: u64, offset: i32 },
}
```

### Status Indication

- Show `[watching]` or `👁` in status bar when enabled
- Brief flash or indicator when file reloads
- Show timestamp of last reload (optional)

### Configuration

```rust
pub struct WatchConfig {
    /// Enable watching by default
    enabled: bool,
    /// Debounce duration
    debounce_ms: u64,
    /// Show reload notification
    notify_on_reload: bool,
    /// Scroll preservation strategy
    scroll_preservation: ScrollPreservation,
}

pub enum ScrollPreservation {
    Heading,
    Percentage,
    ContentHash,
    None,  // Always go to top
}
```

### Key Bindings

| Key | Action |
|-----|--------|
| `w` | Toggle file watching on/off |
| `R` | Force reload (even if watching disabled) |

### Edge Cases

1. **Binary files**: Detect and refuse to watch
2. **Very large files**: Warn about performance, offer to disable
3. **Permissions**: Handle permission changes gracefully
4. **Network files**: May have higher latency, increase debounce
5. **Symlinks**: Watch the target file, not the symlink

---

## Markdown Processing

### Parser: Comrak

We use [comrak](https://github.com/kivikakk/comrak) for markdown parsing because:

1. **Full GFM compatibility** - Used by crates.io, docs.rs, GitLab
2. **AST access** - Enables semantic navigation (jump to headings)
3. **Active maintenance** - Maintainer works on it professionally since Sep 2025
4. **Extensive extensions** - All GFM + useful extras

### Supported Extensions

#### GFM Core (GitHub Flavored Markdown)
| Extension | Syntax | Example |
|-----------|--------|---------|
| Tables | `\| cell \|` | Data tables with alignment |
| Task lists | `- [x]` | Checkboxes in lists |
| Strikethrough | `~~text~~` | ~~deleted~~ |
| Autolinks | `www.example.com` | Auto-linked URLs |
| Fenced code | ` ```lang ` | Syntax highlighted blocks |

#### Additional Extensions
| Extension | Syntax | Rationale |
|-----------|--------|-----------|
| Footnotes | `[^1]` | Common in technical docs |
| Superscript | `e=mc^2^` | Scientific notation |
| Subscript | `H~2~O` | Chemical formulas |
| Heading IDs | `# Title {#custom-id}` | Deep linking |
| Math (display) | `$$...$$` | LaTeX equations (render as code) |
| Math (inline) | `$...$` | Inline equations |
| Description lists | `term\n: definition` | Glossaries |

### Rendering Pipeline

```
Source Text
    │
    ▼
┌─────────────┐
│   Comrak    │ Parse to AST
│   Parser    │
└─────────────┘
    │
    ▼
┌─────────────┐
│    AST      │ Walk and transform
│  Processor  │
└─────────────┘
    │
    ▼
┌─────────────┐
│   Layout    │ Compute line wrapping,
│   Engine    │ indentation, styling
└─────────────┘
    │
    ▼
┌─────────────┐
│  Rendered   │ Cache styled lines
│  Document   │ for fast scrolling
└─────────────┘
```

### Text Wrapping

- Wrap at terminal width minus margins
- Preserve indentation for nested structures
- Soft-wrap paragraphs, hard-wrap code blocks (with horizontal scroll indicator)
- Re-wrap on terminal resize

---

## Image Rendering

### Library: ratatui-image

We use [ratatui-image](https://github.com/benjajaja/ratatui-image) for image rendering because:

1. **Ratatui integration** - Native widget, handles positioning
2. **Protocol detection** - Auto-detects terminal capabilities
3. **Multiple protocols** - Kitty, Sixel, iTerm2, halfblocks
4. **Active development** - Good maintenance

### Protocol Priority

```rust
pub enum ImageProtocol {
    Kitty,      // Best quality, Kitty/Ghostty
    Sixel,      // Good quality, xterm/foot/mlterm
    ITerm2,     // macOS Terminal.app, WezTerm
    Halfblock,  // Universal fallback, unicode ▄
}
```

Detection order:
1. Check `TERM`/`TERM_PROGRAM` environment variables
2. Query terminal with escape sequences
3. Fall back to halfblocks

### Image Handling

```rust
pub struct ImageRef {
    /// Position in source
    source_range: Range<usize>,
    /// Alt text for accessibility/fallback
    alt_text: String,
    /// Image source (path or URL)
    source: ImageSource,
    /// Rendered position in document
    line_range: Range<usize>,
    /// Loaded image data (lazy)
    data: Option<DynamicImage>,
}

pub enum ImageSource {
    LocalPath(PathBuf),
    Url(String),
    DataUri(Vec<u8>),
}
```

### Image Loading Strategy

1. **Lazy loading** - Only load images in/near viewport
2. **Async loading** - Don't block scrolling
3. **Caching** - Keep decoded images in memory (with size limit)
4. **Placeholder** - Show `[Image: alt text]` while loading
5. **Error handling** - Show `[Image not found: path]` on failure

### Size Calculation

- Respect terminal cell dimensions
- Scale images to fit width (configurable max height)
- Maintain aspect ratio
- Account for character cell aspect ratio (~2:1 height:width)

---

## Syntax Highlighting

### Library: Syntect

We use [syntect](https://github.com/trishume/syntect) for syntax highlighting because:

1. **Battle-tested** - Used by xi-editor, bat, many others
2. **Sublime syntax** - Huge library of language definitions
3. **Pure Rust option** - No C dependencies with `fancy-regex`
4. **Theme support** - Many built-in themes

### Configuration

```rust
pub struct HighlightConfig {
    /// Theme name (e.g., "base16-ocean.dark")
    theme: String,
    /// Whether to show line numbers in code blocks
    line_numbers: bool,
    /// Tab width for code
    tab_width: usize,
}
```

### Supported Languages

All languages with Sublime syntax definitions (~150+), including:
- Rust, Go, Python, JavaScript/TypeScript, C/C++
- Shell (bash, zsh, fish)
- Config files (TOML, YAML, JSON)
- Markup (HTML, CSS, XML)
- And many more

### Theme Adaptation

- Detect terminal background (dark/light) when possible
- Provide sensible defaults for 256-color and truecolor terminals
- Allow user theme override via config

---

## Key Bindings

### Navigation

| Key | Action |
|-----|--------|
| `j`, `↓` | Scroll down one line |
| `k`, `↑` | Scroll up one line |
| `h`, `←` | Scroll left (in wide content) |
| `l`, `→` | Scroll right (in wide content) |
| `Space`, `PgDn` | Scroll down one page |
| `b`, `PgUp` | Scroll up one page |
| `Ctrl-d` | Scroll down half page |
| `Ctrl-u` | Scroll up half page |
| `g`, `Home` | Go to beginning |
| `G`, `End` | Go to end |
| `{n}g` | Go to line n |
| `{n}%` | Go to n percent |

### Search

| Key | Action |
|-----|--------|
| `/` | Start forward search |
| `?` | Start backward search |
| `n` | Next match |
| `N` | Previous match |
| `Esc` | Clear search |

### Document Navigation

| Key | Action |
|-----|--------|
| `]h`, `]]` | Next heading |
| `[h`, `[[` | Previous heading |
| `{n}]h` | Next heading level n |
| `Tab` | Next link |
| `Shift-Tab` | Previous link |
| `Enter` | Open link (if supported) |

### Table of Contents

| Key | Action |
|-----|--------|
| `t` | Toggle TOC sidebar |
| `T` | Toggle TOC and focus it |
| `Tab` | Switch focus between TOC and document |
| `Enter` | Jump to selected heading (in TOC) |
| `h`, `←` | Collapse heading (in TOC) |
| `l`, `→` | Expand heading (in TOC) |

### File Watching

| Key | Action |
|-----|--------|
| `w` | Toggle file watching |
| `R` | Force reload file |

### Application

| Key | Action |
|-----|--------|
| `q`, `Ctrl-c` | Quit |
| `?`, `F1` | Show help |
| `r` | Reload file |
| `Ctrl-l` | Redraw screen |

### Mouse (when enabled)

| Action | Effect |
|--------|--------|
| Scroll wheel | Scroll up/down |
| Click on link | Open link |
| Click on heading (TOC) | Jump to heading |

---

## Project Structure

```
gander/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── DESIGN.md              # This document
├── CHANGELOG.md
├── LICENSE
├── rustfmt.toml           # Formatter config
├── clippy.toml            # Linter config
├── .github/
│   └── workflows/
│       ├── ci.yml         # Test, lint, format check
│       └── release.yml    # Binary releases
├── src/
│   ├── main.rs            # Entry point, CLI
│   ├── lib.rs             # Library root
│   ├── app.rs             # Application state, TEA loop
│   ├── config.rs          # Configuration handling
│   ├── document/
│   │   ├── mod.rs
│   │   ├── parser.rs      # Markdown parsing (comrak)
│   │   ├── ast.rs         # AST types and traversal
│   │   ├── layout.rs      # Line wrapping, rendering
│   │   └── rendered.rs    # Rendered document types
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── viewport.rs    # Viewport management
│   │   ├── layout.rs      # Split layouts (TOC + document)
│   │   ├── widgets/
│   │   │   ├── mod.rs
│   │   │   ├── document.rs    # Main document widget
│   │   │   ├── toc.rs         # Table of contents sidebar
│   │   │   ├── status_bar.rs  # Bottom status line
│   │   │   ├── search_bar.rs  # Search input
│   │   │   └── help.rs        # Help overlay
│   │   └── style.rs       # Theming, colors
│   ├── watcher/
│   │   ├── mod.rs
│   │   ├── debounce.rs    # Debounce logic
│   │   └── anchor.rs      # Scroll position preservation
│   ├── image/
│   │   ├── mod.rs
│   │   ├── loader.rs      # Async image loading
│   │   ├── cache.rs       # Image cache
│   │   └── protocol.rs    # Protocol detection
│   ├── highlight/
│   │   ├── mod.rs
│   │   └── theme.rs       # Syntect theme handling
│   ├── input/
│   │   ├── mod.rs
│   │   ├── handler.rs     # Event -> Message mapping
│   │   └── keybindings.rs # Configurable bindings
│   └── search/
│       ├── mod.rs
│       └── matcher.rs     # Search implementation
├── tests/
│   ├── integration/
│   │   ├── mod.rs
│   │   ├── document_tests.rs
│   │   ├── navigation_tests.rs
│   │   └── rendering_tests.rs
│   └── fixtures/
│       ├── simple.md
│       ├── complex.md
│       ├── with_images.md
│       └── edge_cases.md
├── benches/
│   ├── parsing.rs
│   └── rendering.rs
└── docs/
    └── examples/
        └── sample.md
```

---

## Dependencies

### Core Dependencies

```toml
[dependencies]
# TUI framework
ratatui = "0.29"
crossterm = "0.28"

# Markdown parsing
comrak = { version = "0.31", default-features = false }

# Image rendering
ratatui-image = "3"
image = "0.25"

# Syntax highlighting
syntect = { version = "5", default-features = false, features = ["default-fancy"] }

# CLI argument parsing
clap = { version = "4", features = ["derive"] }

# Error handling
thiserror = "2"
anyhow = "1"

# Async runtime (for image loading, file watching)
tokio = { version = "1", features = ["rt", "fs", "sync", "time"] }

# File watching
notify = "7"
notify-debouncer-mini = "0.5"

# Logging (optional, for debugging)
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### Dev Dependencies

```toml
[dev-dependencies]
# Testing
pretty_assertions = "1"
insta = "1"           # Snapshot testing
proptest = "1"        # Property-based testing
test-case = "3"       # Parameterized tests
mockall = "0.13"      # Mocking

# Benchmarking
criterion = "0.5"
```

### Build Dependencies

```toml
[build-dependencies]
# None needed initially
```

### Feature Flags

```toml
[features]
default = ["sixel", "kitty"]
sixel = ["ratatui-image/sixel"]
kitty = ["ratatui-image/kitty"]
```

---

## Testing Strategy

### Red-Green-Refactor TDD

We follow strict TDD:

1. **Red** - Write a failing test first
2. **Green** - Write minimal code to pass
3. **Refactor** - Clean up while tests pass

### Test Categories

#### Unit Tests (in `src/`)

Test individual functions and types in isolation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_visible_range_at_top() {
        let viewport = Viewport::new(80, 24, 100);
        assert_eq!(viewport.visible_range(), 0..24);
    }

    #[test]
    fn viewport_visible_range_scrolled() {
        let mut viewport = Viewport::new(80, 24, 100);
        viewport.scroll_to(50);
        assert_eq!(viewport.visible_range(), 50..74);
    }
}
```

#### Integration Tests (in `tests/`)

Test component interactions:

```rust
#[test]
fn parse_and_render_basic_document() {
    let md = "# Hello\n\nThis is a paragraph.";
    let doc = Document::parse(md).unwrap();
    let rendered = doc.render(80);

    assert_eq!(rendered.lines().len(), 3);
    assert_eq!(rendered.lines()[0].line_type, LineType::Heading(1));
}
```

#### Snapshot Tests (using insta)

Capture rendered output for regression testing:

```rust
#[test]
fn render_complex_document() {
    let md = include_str!("../fixtures/complex.md");
    let doc = Document::parse(md).unwrap();
    let rendered = doc.render(80);

    insta::assert_snapshot!(rendered.to_string());
}
```

#### Property-Based Tests (using proptest)

Test invariants with random input:

```rust
proptest! {
    #[test]
    fn scroll_position_never_exceeds_bounds(
        total_lines in 1..10000usize,
        height in 1..100u16,
        scroll_amount in 0..10000usize,
    ) {
        let mut viewport = Viewport::new(80, height, total_lines);
        viewport.scroll_down(scroll_amount);

        assert!(viewport.offset + viewport.height as usize <= total_lines);
    }
}
```

### Test Coverage Goals

| Component | Target Coverage |
|-----------|-----------------|
| Document parsing | 90%+ |
| Viewport/scrolling | 95%+ |
| Event handling | 85%+ |
| Image loading | 70%+ (async is tricky) |
| UI rendering | 60%+ (snapshot tests) |

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test viewport_visible_range

# Run with coverage
cargo llvm-cov

# Run benchmarks
cargo bench
```

---

## Development Tooling

### Formatting: rustfmt

Configuration (`rustfmt.toml`):

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
use_small_heuristics = "Default"
imports_granularity = "Module"
group_imports = "StdExternalCrate"
```

Run: `cargo fmt`
Check: `cargo fmt --check`

### Linting: Clippy

Configuration (`clippy.toml`):

```toml
cognitive-complexity-threshold = 25
too-many-arguments-threshold = 7
```

Workspace lints (`Cargo.toml`):

```toml
[lints.rust]
unsafe_code = "forbid"

[lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
cargo = "warn"

# Allow some pedantic lints
module_name_repetitions = "allow"
must_use_candidate = "allow"
```

Run: `cargo clippy`
Fix: `cargo clippy --fix`

### Code Coverage: cargo-llvm-cov

```bash
# Install
cargo install cargo-llvm-cov

# Run with HTML report
cargo llvm-cov --html

# Run with threshold check (for CI)
cargo llvm-cov --fail-under-lines 70
```

### Documentation

```bash
# Build docs
cargo doc

# Build and open
cargo doc --open

# Include private items (for development)
cargo doc --document-private-items
```

Documentation style:
- All public items must have doc comments
- Examples in doc comments where appropriate
- Module-level documentation explaining purpose

### CI Pipeline (GitHub Actions)

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test

  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - run: cargo clippy -- -D warnings

  format:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --check

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: taiki-e/install-action@cargo-llvm-cov
      - run: cargo llvm-cov --fail-under-lines 70

  docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo doc --no-deps
```

### Pre-commit Hooks (optional)

```bash
#!/bin/sh
# .git/hooks/pre-commit

cargo fmt --check || exit 1
cargo clippy -- -D warnings || exit 1
cargo test || exit 1
```

---

## Future Considerations

### Potential Features (Post-MVP)

1. **Multiple files** - Tab or split view
2. **Link following** - Open URLs in browser
3. **Export** - Copy rendered output, maybe HTML
4. **Configuration file** - `~/.config/gander/config.toml`
5. **Custom themes** - User-defined color schemes
6. **Vim-style marks** - `m{a-z}` to set, `'{a-z}` to jump
7. **Shell integration** - `man` replacement mode
8. **Remote files** - View markdown from URLs

### Performance Optimizations

1. **Incremental parsing** - For very large files
2. **Virtual scrolling** - Only render visible lines
3. **Background image loading** - Non-blocking with progress
4. **Mmap for large files** - Memory-efficient reading

### Accessibility

1. **Screen reader support** - Alt text for images
2. **High contrast themes** - For vision impairment
3. **Configurable font size** - Via terminal

---

## References

### Libraries

- [comrak](https://github.com/kivikakk/comrak) - Markdown parser
- [ratatui](https://ratatui.rs/) - TUI framework
- [ratatui-image](https://github.com/benjajaja/ratatui-image) - Image rendering
- [syntect](https://github.com/trishume/syntect) - Syntax highlighting
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal backend

### Specifications

- [CommonMark Spec](https://spec.commonmark.org/)
- [GitHub Flavored Markdown Spec](https://github.github.com/gfm/)
- [Kitty Graphics Protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
- [Sixel Graphics](https://en.wikipedia.org/wiki/Sixel)

### Similar Tools

- [mdless](https://crates.io/crates/mdless) - Terminal markdown viewer
- [glow](https://github.com/charmbracelet/glow) - Go-based markdown viewer
- [bat](https://github.com/sharkdp/bat) - Cat clone with syntax highlighting
- [less](https://www.greenwoodsoftware.com/less/) - The classic pager
