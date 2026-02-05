# Gander - Claude Code Instructions

**Follow Strict RED/GREEN TDD in all development.**

1. Write a failing test FIRST that describes the behavior you want
2. Run the test - watch it FAIL (RED)
3. Write the minimum code to make it pass (GREEN)
4. Refactor while keeping tests green
5. NEVER write feature code without a failing test first

## Project Overview

Gander is a Rust TUI markdown viewer with:
- Image support (Kitty, Sixel, half-block fallback)
- Table of contents sidebar
- File watching / live reload
- Syntax-highlighted code blocks

## Architecture

### The Elm Architecture (TEA)

This project uses TEA pattern. All state changes flow through:

```
Event -> Message -> update(Model, Message) -> Model -> view(Model) -> Frame
```

**Rules:**
- `update()` must be a pure function - no side effects
- Side effects (file I/O, terminal queries) happen in the event loop, producing Messages
- State lives in `Model`, not scattered across components

### Module Organization

```
src/
├── app.rs          # TEA loop, Model, Message, update()
├── document/       # Markdown parsing and rendering
├── ui/             # Ratatui widgets and layout
├── image/          # Image loading and protocol detection
├── watcher/        # File watching
├── highlight/      # Syntax highlighting
├── input/          # Event handling, keybindings
└── search/         # Search functionality
```

## Development Practices

### Red-Green-Refactor TDD

**Always write tests first:**

1. Write a failing test
2. Write minimal code to pass
3. Refactor while green

```rust
// Example: Start with the test
#[test]
fn viewport_clamps_scroll_to_bounds() {
    let mut vp = Viewport::new(80, 24, 100);
    vp.scroll_down(1000);
    assert_eq!(vp.offset, 76); // 100 - 24 = 76 max
}
```

### Test Commands

```bash
cargo test                    # Run all tests
cargo test viewport           # Run tests matching "viewport"
cargo test -- --nocapture     # Show println! output
cargo llvm-cov                # Coverage report
cargo llvm-cov --html         # HTML coverage report
```

### Linting & Formatting

```bash
cargo fmt                     # Format code
cargo fmt --check             # Check formatting (CI)
cargo clippy                  # Lint
cargo clippy -- -D warnings   # Lint, fail on warnings (CI)
```

### Documentation

```bash
cargo doc --open              # Build and view docs
```

All public items must have doc comments with examples where appropriate.

## Code Style

### Error Handling

- Use `thiserror` for library errors (typed, specific)
- Use `anyhow` for application errors (convenient, contextual)
- Never `unwrap()` in library code; ok in tests

```rust
// Good
pub fn parse(input: &str) -> Result<Document, ParseError>

// In application code
let doc = parse(&content).context("Failed to parse markdown")?;
```

### Naming Conventions

- Types: `PascalCase`
- Functions/methods: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Modules: `snake_case`
- Test functions: `test_<what>_<condition>_<expected>`

```rust
#[test]
fn test_viewport_scroll_down_clamps_to_max() { }
```

### Imports

Group imports in this order (rustfmt handles this):
1. `std`
2. External crates
3. Crate modules (`crate::`)

## Key Types

```rust
// Core state
struct Model {
    document: RenderedDocument,
    viewport: Viewport,
    toc_visible: bool,
    watch_enabled: bool,
    // ...
}

// All user actions and events
enum Message {
    ScrollUp(usize),
    ScrollDown(usize),
    ToggleToc,
    FileChanged,
    // ...
}

// Document line types for styling
enum LineType {
    Paragraph,
    Heading(u8),
    CodeBlock,
    // ...
}
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | TUI framework |
| `crossterm` | Terminal backend |
| `comrak` | Markdown parsing (GFM compatible) |
| `ratatui-image` | Terminal image rendering |
| `syntect` | Syntax highlighting |
| `notify` | File watching |
| `clap` | CLI argument parsing |
| `tokio` | Async runtime |

## Common Tasks

### Adding a New Message Type

1. Add variant to `Message` enum in `app.rs`
2. Handle in `update()` function
3. Add keybinding in `input/handler.rs`
4. Write tests for the state transition

### Adding a New Widget

1. Create file in `src/ui/widgets/`
2. Implement `Widget` or `StatefulWidget` trait
3. Add to `view()` function layout
4. Write snapshot tests with `insta`

### Adding Markdown Extension Support

1. Enable in comrak `ExtensionOptions`
2. Handle new AST node types in `document/parser.rs`
3. Add rendering logic in `document/layout.rs`
4. Add test fixtures in `tests/fixtures/`

## Testing Patterns

### Unit Tests (in module)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specific_behavior() {
        // Arrange
        let input = "...";
        // Act
        let result = function_under_test(input);
        // Assert
        assert_eq!(result, expected);
    }
}
```

### Snapshot Tests (for rendering)

```rust
#[test]
fn render_heading() {
    let doc = Document::parse("# Hello").unwrap();
    let rendered = doc.render(80);
    insta::assert_snapshot!(rendered.to_string());
}
```

### Property Tests (for invariants)

```rust
proptest! {
    #[test]
    fn scroll_never_negative(offset in 0..1000usize) {
        let mut vp = Viewport::new(80, 24, 100);
        vp.scroll_up(offset);
        assert!(vp.offset < 100);
    }
}
```

## File Watching Notes

- Use `notify` crate with debouncing (100ms default)
- Preserve scroll position on reload using heading anchors
- Show `[watching]` indicator in status bar
- Handle file deletion gracefully (keep content, show warning)

## Image Rendering Notes

- Protocol detection order: Kitty > Sixel > iTerm2 > Halfblock
- Lazy load images (only in/near viewport)
- Cache decoded images with size limit
- Show `[Image: alt text]` placeholder while loading

## TOC Sidebar Notes

- Extract headings during parse phase
- Build tree structure from flat heading list
- Sync current heading highlight while scrolling
- Support collapse/expand with `h`/`l` keys
- Default width: 25% of terminal or 30 chars minimum
