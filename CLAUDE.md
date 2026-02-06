# Markless - Claude Code Instructions

**Follow Strict RED/GREEN TDD in all development.**

1. Write a failing test FIRST that describes the behavior you want
2. Run the test - watch it FAIL (RED)
3. Write the minimum code to make it pass (GREEN)
4. Refactor while keeping tests green
5. NEVER write feature code without a failing test first

## Project Overview

Markless is a Rust TUI markdown viewer with:
- Image support (Kitty, Sixel, iTerm2, half-block fallback)
- Table of contents sidebar
- File watching / live reload
- Syntax-highlighted code blocks
- Search and mouse selection + copy

## Architecture

### The Elm Architecture (TEA)

This project uses TEA. All state changes flow through:

```
Event -> Message -> update(Model, Message) -> Model -> view(Model) -> Frame
```

**Rules:**
- `update()` must be a pure function - no side effects
- Side effects (file I/O, external open, clipboard) happen in `app/effects.rs`
- State lives in `Model`

### Module Organization

```
src/
├── app/           # TEA model, update, input, effects, event loop
├── config.rs      # CLI flags + config persistence
├── document/      # Markdown parsing and rendered document types
├── highlight/     # Syntax highlighting (syntect)
├── image/         # Image loading and protocol detection
├── input/         # Low-level input helpers
├── perf.rs        # Performance logging
├── search/        # Search helpers
├── ui/            # Rendering, overlays, styles
└── watcher/       # File watching wrapper
```

## Development Practices

### Red-Green-Refactor TDD

Always write tests first.

### Test Commands

```bash
cargo test                    # Run all tests
cargo test <pattern>          # Run matching tests
cargo test -- --nocapture     # Show println! output
```

### Linting & Formatting

```bash
cargo fmt                     # Format code
cargo fmt --check             # Check formatting (CI)
cargo clippy                  # Lint
cargo clippy -- -D warnings   # Lint, fail on warnings (CI)
./scripts/check.sh            # Run all local checks (format + tests)
```

A pre-commit hook auto-formats staged `.rs` files. To enable:

```bash
git config core.hooksPath .githooks
```

### Documentation

```bash
cargo doc --open              # Build and view docs
```

All public items must have doc comments with examples where appropriate.

## Code Style

### Error Handling

- Use `anyhow` for application errors
- Avoid `unwrap()` in library code (ok in tests)

### Naming Conventions

- Types: `PascalCase`
- Functions/methods: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Test functions: `test_<what>_<condition>_<expected>`

### Imports

Group imports in this order:
1. `std`
2. External crates
3. `crate::...`

## Key Types

```rust
struct Model {
    document: Document,
    viewport: Viewport,
    toc_visible: bool,
    watch_enabled: bool,
    // ...
}

enum Message {
    ScrollUp(usize),
    ScrollDown(usize),
    ToggleToc,
    FileChanged,
    // ...
}

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
| `comrak` | Markdown parsing |
| `ratatui-image` | Terminal image rendering |
| `syntect` | Syntax highlighting |
| `notify` | File watching |
| `clap` | CLI argument parsing |

## Common Tasks

### Adding a New Message Type

1. Add variant to `Message` in `src/app/update.rs`
2. Handle it in `update()`
3. Add input mapping in `src/app/input.rs`
4. Write tests for the state transition

### Adding a New UI Element

1. Implement rendering in `src/ui/render.rs` or `src/ui/overlays.rs`
2. Add state to `Model` if needed
3. Add tests (unit or UI buffer tests)

### Adding Markdown Support

1. Enable in comrak `Options` (see `src/document/parser.rs`)
2. Handle AST nodes in the parser
3. Add tests in `src/document/parser.rs`

## Notes

- Image placeholders are `[Image: Alt text]` when images are not rendered.
- Image captions (alt text) render above images when heights are known.
- Selection is line-based and copies text as a fixed-width block; links copy as URLs.
