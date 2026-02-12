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

use std::path::PathBuf;
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

/// A terminal markdown viewer with image support
#[derive(Parser, Debug)]
#[command(name = "markless", version, about, long_about = None)]
struct Cli {
    /// Markdown file or directory to view
    #[arg(value_name = "PATH", default_value = ".")]
    path: PathBuf,

    /// Watch file for changes and auto-reload
    #[arg(short, long)]
    watch: bool,

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
    use std::io::{Read, Write};
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();

    let mut io = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")?;
    let reader = io.try_clone()?;

    // OSC 11 query: ESC ] 11 ; ? BEL
    io.write_all(b"\x1b]11;?\x07")?;
    io.flush()?;

    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 256];
        let mut collected: Vec<u8> = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => {
                    collected.extend_from_slice(&buf[..n]);
                    if collected.contains(&b'\x07') || collected.windows(2).any(|w| w == b"\x1b\\")
                    {
                        let _ = tx.send(collected);
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let collected: Vec<u8> = rx
        .recv_timeout(Duration::from_millis(75))
        .unwrap_or_default();

    let mut found: Option<(u8, u8, u8)> = None;
    if !collected.is_empty() {
        let text = String::from_utf8_lossy(&collected);
        if text.contains("rgb:") {
            found = parse_osc11_reply(&text);
        }
    }

    Ok(found)
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

fn parse_osc11_reply(reply: &str) -> Option<(u8, u8, u8)> {
    // Expect: ESC ] 11 ; rgb:RRRR/GGGG/BBBB BEL or ST
    let start = reply.find("rgb:")?;
    let data = &reply[start + 4..];
    let mut parts = data.split(['/', '\x07', '\x1b']);
    let r = parts.next()?;
    let g = parts.next()?;
    let b = parts.next()?;
    Some((
        parse_osc_component(r)?,
        parse_osc_component(g)?,
        parse_osc_component(b)?,
    ))
}

fn parse_osc_component(s: &str) -> Option<u8> {
    let hex = s.trim();
    if hex.len() >= 4 {
        let v = u16::from_str_radix(&hex[..4], 16).ok()?;
        Some((v >> 8) as u8)
    } else if hex.len() == 2 {
        u8::from_str_radix(hex, 16).ok()
    } else {
        None
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

    // Verify path exists
    if !cli.path.exists() {
        anyhow::bail!("Path not found: {}", cli.path.display());
    }

    let is_directory = cli.path.is_dir();

    // Run the application
    // Normalize editor: empty string from --no-editor becomes None
    let editor = effective.editor.filter(|e| !e.is_empty());

    let mut app = App::new(cli.path)
        .with_watch(effective.watch)
        .with_toc_visible(effective.toc && !effective.no_toc)
        .with_image_mode(effective.image_mode)
        .with_images_enabled(!effective.no_images)
        .with_browse_mode(is_directory)
        .with_wrap_width(effective.wrap_width)
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
