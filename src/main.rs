#![allow(clippy::multiple_crate_versions)]

//! Markless - A terminal markdown viewer with image support.
//!
//! # Usage
//!
//! ```bash
//! markless README.md
//! markless --watch README.md
//! markless --no-toc README.md
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use markless::app::App;
use markless::config::{
    ConfigFlags, ImageMode, ThemeMode, clear_config_flags, global_config_path, load_config_flags,
    local_override_path, parse_flag_tokens, save_config_flags,
};
use markless::highlight::{HighlightBackground, set_background_mode};
use markless::perf;
#[cfg(unix)]
use markless::term_query::{parse_osc11_reply, read_osc_response};

/// A terminal markdown viewer with image support
#[derive(Parser, Debug)]
#[command(name = "markless", version, about, long_about = None)]
struct Cli {
    /// Markdown file, directory to browse, or `-` to read markdown from stdin
    #[arg(value_name = "PATH", default_value = ".")]
    path: PathBuf,

    /// Watch file for changes and auto-reload
    #[arg(short, long)]
    watch: bool,

    /// Disable markless mouse selection capture
    #[arg(long)]
    no_mouse_select: bool,

    /// Re-enable markless mouse selection capture (overrides saved --no-mouse-select)
    #[arg(long, conflicts_with = "no_mouse_select")]
    mouse_select: bool,

    /// Hide table of contents sidebar
    #[arg(long)]
    no_toc: bool,

    /// Start with TOC sidebar visible
    #[arg(long)]
    toc: bool,

    /// Disable inline image rendering (show placeholders only)
    #[arg(long)]
    no_images: bool,

    /// Force syntax highlight theme background (light or dark)
    #[arg(long, value_enum, default_value = "auto")]
    theme: ThemeMode,

    /// Enable startup performance logging
    #[arg(long)]
    perf: bool,

    /// Write detailed render/image debug events to a file
    #[arg(long, value_name = "PATH")]
    render_debug_log: Option<PathBuf>,

    /// Force image rendering to use half-cell fallback mode
    #[arg(long)]
    force_half_cell: bool,

    /// Force a specific image rendering mode (kitty, sixel, iterm2, halfblock)
    #[arg(long, value_enum)]
    image_mode: Option<ImageMode>,

    /// Maximum content width for word wrapping (in columns)
    #[arg(long, value_name = "COLS")]
    wrap_width: Option<u16>,

    /// External editor command (e.g. hx, vim, "emacsclient -t")
    #[arg(long, value_name = "COMMAND")]
    editor: Option<String>,

    /// Clear external editor setting (revert to built-in editor)
    #[arg(long, conflicts_with = "editor")]
    no_editor: bool,

    /// Disable inline (Unicode) math, rendering as images instead
    #[arg(long)]
    no_inline_math: bool,

    /// Re-enable inline (Unicode) math (overrides saved --no-inline-math)
    #[arg(long, conflicts_with = "no_inline_math")]
    inline_math: bool,

    /// Save current command-line flags as defaults in .marklessrc
    #[arg(long)]
    save: bool,

    /// Clear saved defaults in .marklessrc
    #[arg(long)]
    clear: bool,
}

// Query the terminal background using OSC 11.
// We talk to /dev/tty so the terminal responds even when stdout is piped.
// On non-Unix platforms we skip the query entirely because the fallback
// (stdin/stdout) leaves an orphaned reader thread that blocks the console
// input buffer, preventing crossterm from receiving any keyboard events.
#[cfg(not(unix))]
fn query_terminal_background() -> std::io::Result<Option<(u8, u8, u8)>> {
    Ok(None)
}

#[cfg(unix)]
fn query_terminal_background() -> std::io::Result<Option<(u8, u8, u8)>> {
    use std::os::unix::fs::OpenOptionsExt;

    // Open /dev/tty non-blocking so the bounded read in
    // `read_osc_response` can poll for the reply without spawning a
    // background reader that would race crossterm for keystrokes on
    // terminals that don't reply to OSC 11 (issue #53).
    let mut io = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/dev/tty")?;

    let collected = read_osc_response(&mut io, Duration::from_millis(75));
    if collected.is_empty() {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&collected);
    if !text.contains("rgb:") {
        return Ok(None);
    }
    Ok(parse_osc11_reply(&text))
}

fn theme_from_rgb(r: u8, g: u8, b: u8) -> HighlightBackground {
    let luma = 0.2126f32.mul_add(
        f32::from(r),
        0.7152f32.mul_add(f32::from(g), 0.0722 * f32::from(b)),
    );
    if luma >= 140.0 {
        HighlightBackground::Light
    } else {
        HighlightBackground::Dark
    }
}

fn detect_theme() -> Option<HighlightBackground> {
    let _raw = enable_raw_mode();
    let result = query_terminal_background();
    let _ = disable_raw_mode();
    result
        .ok()
        .flatten()
        .map(|(r, g, b)| theme_from_rgb(r, g, b))
}

fn relaunch_with_theme(mode: HighlightBackground, raw_args: &[String]) -> Result<()> {
    let exe = std::env::current_exe().context("current exe")?;
    let tokens = raw_args.get(1..).unwrap_or_default();
    let mut args: Vec<String> = Vec::with_capacity(tokens.len() + 2);
    let mut i = 0;
    let mut saw_theme = false;
    while i < tokens.len() {
        let token = &tokens[i];
        if token == "--theme" {
            saw_theme = true;
            i += 1;
            if i < tokens.len() {
                i += 1;
            }
            args.push("--theme".to_string());
            args.push(match mode {
                HighlightBackground::Light => "light".to_string(),
                HighlightBackground::Dark => "dark".to_string(),
            });
            continue;
        }
        if let Some(value) = token.strip_prefix("--theme=") {
            saw_theme = true;
            if value == "auto" {
                args.push(format!(
                    "--theme={}",
                    match mode {
                        HighlightBackground::Light => "light",
                        HighlightBackground::Dark => "dark",
                    }
                ));
            } else {
                args.push(token.clone());
            }
            i += 1;
            continue;
        }
        args.push(token.clone());
        i += 1;
    }

    if !saw_theme {
        args.push("--theme".to_string());
        args.push(match mode {
            HighlightBackground::Light => "light".to_string(),
            HighlightBackground::Dark => "dark".to_string(),
        });
    }

    let status = Command::new(exe).args(args).status()?;
    if !status.success() {
        anyhow::bail!("failed to relaunch markless with detected theme");
    }
    Ok(())
}

/// Resolve the CLI path argument, mapping the stdin sentinel `-` to the
/// pseudo-path `<stdin>` used throughout the app.
///
/// Returns the resolved path and whether the document should be read from
/// standard input (`markless -`).
fn resolve_path(path: PathBuf) -> (PathBuf, bool) {
    if path == Path::new("-") {
        (PathBuf::from("<stdin>"), true)
    } else {
        (path, false)
    }
}

fn main() -> Result<()> {
    // Restore terminal state on panic so the shell isn't left in raw mode
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(std::io::stderr(), crossterm::terminal::LeaveAlternateScreen);
        default_hook(info);
    }));

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let raw_args = std::env::args().collect::<Vec<_>>();
    let cli = Cli::parse();
    let (path, stdin_mode) = resolve_path(cli.path.clone());
    let global_path = global_config_path();
    let local_path = local_override_path();
    let cli_flags = parse_flag_tokens(&raw_args);

    if cli.clear {
        clear_config_flags(&global_path)?;
    }
    if cli.save {
        save_config_flags(&global_path, &cli_flags)?;
    }

    let file_flags = if cli.clear {
        ConfigFlags::default()
    } else {
        let global_flags = load_config_flags(&global_path)?;
        let local_flags = load_config_flags(&local_path)?;
        global_flags.union(&local_flags)
    };
    let effective = file_flags.union(&cli_flags);

    perf::set_enabled(effective.perf);
    let render_debug_log_path = effective
        .render_debug_log
        .clone()
        .or_else(|| std::env::var_os("MARKLESS_RENDER_DEBUG_LOG").map(PathBuf::from));
    if let Err(err) = perf::set_debug_log_path(render_debug_log_path.as_deref()) {
        eprintln!(
            "[warn] Failed to initialize render debug log {}: {}",
            render_debug_log_path
                .as_ref()
                .map_or_else(|| "<unset>".to_string(), |p| p.display().to_string()),
            err
        );
    }

    match effective.theme.unwrap_or(ThemeMode::Auto) {
        ThemeMode::Auto => {
            if let Some(mode) = detect_theme() {
                return relaunch_with_theme(mode, &raw_args);
            }
            set_background_mode(None);
        }
        ThemeMode::Light => set_background_mode(Some(HighlightBackground::Light)),
        ThemeMode::Dark => set_background_mode(Some(HighlightBackground::Dark)),
    }

    // Verify path exists (skipped for `markless -`, which reads stdin)
    if !stdin_mode && !cli.path.exists() {
        anyhow::bail!("Path not found: {}", cli.path.display());
    }

    let is_directory = !stdin_mode && cli.path.is_dir();

    // Run the application
    // Normalize editor: empty string from --no-editor becomes None
    let editor = effective.editor.filter(|e| !e.is_empty());

    let mut app = App::new(path)
        .with_stdin_mode(stdin_mode)
        .with_watch(effective.watch)
        .with_mouse_enabled(effective.mouse_select || !effective.no_mouse_select)
        .with_toc_visible(effective.toc && !effective.no_toc)
        .with_image_mode(effective.image_mode)
        .with_images_enabled(!effective.no_images)
        .with_browse_mode(is_directory)
        .with_wrap_width(effective.wrap_width)
        .with_no_inline_math(effective.no_inline_math && !effective.inline_math)
        .with_editor(editor)
        .with_config_paths(
            Some(global_path),
            if local_path.exists() {
                Some(local_path)
            } else {
                None
            },
        );

    app.run().context("Application error")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn dash_path_resolves_to_stdin_mode() {
        let (path, stdin_mode) = resolve_path(PathBuf::from("-"));
        assert!(stdin_mode, "`-` should enable stdin mode");
        assert_eq!(path, PathBuf::from("<stdin>"));
    }

    #[test]
    fn ordinary_path_is_not_stdin_mode() {
        let (path, stdin_mode) = resolve_path(PathBuf::from("README.md"));
        assert!(!stdin_mode);
        assert_eq!(path, PathBuf::from("README.md"));
    }

    #[test]
    fn default_path_is_current_directory() {
        let cli = Cli::try_parse_from(["markless"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("."));
    }

    #[test]
    fn cli_accepts_dash_as_path() {
        let cli = Cli::try_parse_from(["markless", "-"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("-"));
    }

    #[test]
    fn cli_accepts_flags_with_dash_path() {
        let cli = Cli::try_parse_from(["markless", "--no-toc", "-"]).unwrap();
        assert!(cli.no_toc);
        assert_eq!(cli.path, PathBuf::from("-"));
    }
}
