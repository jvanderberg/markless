# Gander - Design Document

This document describes the current architecture and behavior of Gander as implemented in the codebase. It is intentionally pragmatic and scoped to what exists today.

## Overview

Gander is a terminal markdown viewer that renders CommonMark with selected GFM extensions, supports inline images, and provides fast navigation for long documents. It uses ratatui/crossterm for UI, comrak for markdown parsing, syntect for syntax highlighting, and notify for file watching.

## Goals

- Fast startup and responsive scrolling on large files
- Clear, stable rendering that reflows on terminal resize
- Image support with graceful fallback
- Predictable navigation with keyboard and mouse
- Minimal, testable state transitions (TEA)

## Non-Goals

- Editing or authoring content
- Export to other formats
- Custom markdown extensions beyond CommonMark + enabled GFM features
- GUI support

## Architecture

The app follows The Elm Architecture (TEA):

- **Model**: all application state
- **Message**: user/system events
- **Update**: pure function that transforms state
- **View**: rendering via ratatui

Main flow:

1. Load file into `Document`
2. Create `Model` and run the event loop
3. Handle input to produce `Message`
4. Apply `update` to mutate `Model`
5. Render the UI

Side effects (file reloads, external link open, clipboard copy) are handled in `app/effects.rs`.

## Core Data Model

### Model (src/app/model.rs)

- `document`: parsed and rendered markdown
- `viewport`: scroll + size management
- `toc_visible`, `toc_selected`, `toc_focused`
- `search_query`, `search_matches`, `search_match_index`
- `hovered_link_url`
- `selection`: line-based mouse selection
- `watch_enabled`
- `images_enabled`
- Image caches and layout state

### Document (src/document/types.rs)

- `lines`: rendered lines (with optional styled spans)
- `headings`: TOC entries
- `images`: image references with line ranges
- `links`: link references by rendered line
- `footnotes`: definition lookup

`RenderedLine` tracks text, line type, and optional inline spans.

## Input and Interaction

### Keyboard

Key bindings are handled in `src/app/input.rs` and documented in the README. Important groups:

- Navigation: `j/k`, arrows, page/half-page, `g/G`
- Search: `/`, `Enter` (next), `Esc` (clear)
- TOC: `t`, `T`, `Tab`, `Enter`/`Space`
- Other: `w`, `r/R`, `o`, `?`/`F1`, `q`/`Ctrl-c`

### Mouse

- Scroll wheel: scroll
- Click: follow links or image placeholders
- Hover: show URL for links and images
- Click+drag: select whole lines and copy

Selection is line-based and copies text as fixed-width blocks. Inline link labels are copied as URLs. Code block borders are stripped during copy.

## Rendering

### Layout

The UI has two primary regions:

- Optional TOC sidebar
- Document view

Footer rows are reserved for status, search, toast, and hover link bar.

### Document Rendering

`render_document` builds a list of `Line` items with styles derived from `LineType` and inline spans. Search matches are highlighted. Selected lines render with a background tint.

### Lists

List items render with hanging indents and have a trailing blank line after the list to provide visual separation.

### Horizontal Rules

Horizontal rules render as a short light line: `─────`.

## Images

### Rendering

Images are rendered via `ratatui-image` using protocol detection. Supported protocols:

- Kitty
- Sixel
- iTerm2
- Half-block fallback

Images are scaled to ~65% of the document width. Image protocol dimensions determine the reserved row height for layout.

### Captions

When an image height is known (rendered images), the image alt text is rendered **above** the image, indented by four spaces. The caption is omitted when images are not rendered.

### No-Images Mode

`--no-images` disables inline rendering and shows placeholders only.

## Links and Footnotes

Links and footnotes are tracked by rendered line for hover/click. Footnote references render with superscript for numeric labels. Footnote definitions render the same label format without colons and with a trailing space.

## File Watching

File watching uses `notify`. When enabled, changes reload the document and reflow the layout.

## Configuration

Flags can be saved to a global config and overridden locally.

Global (macOS): `~/Library/Application Support/gander/config`
Local: `.ganderrc` in the current directory

Saved flags are merged with CLI flags (CLI wins on conflicting options).

## Command Line Options

- `--watch`
- `--no-toc`
- `--toc`
- `--no-images`
- `--force-half-cell`
- `--theme <auto|light|dark>`
- `--perf`
- `--render-debug-log <PATH>`
- `--save`
- `--clear`

## Project Structure

```
src/
  app/           TEA model, update, input, effects, event loop
  config.rs      Flag parsing and config persistence
  document/      Markdown parsing and rendered document types
  highlight/     Syntect integration
  image/         Image loading and terminal protocol selection
  input/         Low-level input handling helpers
  perf.rs        Performance logging utilities
  search/        Search and match helpers
  ui/            Rendering, overlays, and styling
  watcher/       File watcher wrapper
```

## Testing Strategy

Tests live alongside modules and focus on:

- Parsing and rendering of markdown primitives
- TEA state transitions
- Input handling
- Image layout behavior

The project follows strict TDD (red/green/refactor).

## Future Considerations

- Improved link picking UX
- More granular theme controls
- Additional render performance profiling
