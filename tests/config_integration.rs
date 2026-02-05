use std::path::PathBuf;

use gander::config::{
    load_config_flags, parse_flag_tokens, ConfigFlags, ThemeMode,
};

#[test]
fn test_config_file_parsing_ignores_comments_and_blank_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".ganderrc");
    let content = r#"
# comment
--watch

--theme light
   
--render-debug-log=render.log
"#;
    std::fs::write(&path, content).unwrap();

    let flags = load_config_flags(&path).unwrap();
    assert!(flags.watch);
    assert_eq!(flags.theme, Some(ThemeMode::Light));
    assert_eq!(flags.render_debug_log, Some(PathBuf::from("render.log")));
}

#[test]
fn test_cli_flags_override_file_flags() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".ganderrc");
    let content = "--watch\n--theme light\n--render-debug-log file.log\n";
    std::fs::write(&path, content).unwrap();

    let file_flags = load_config_flags(&path).unwrap();
    let cli_args = vec![
        "gander".to_string(),
        "--theme".to_string(),
        "dark".to_string(),
        "--force-half-cell".to_string(),
    ];
    let cli_flags = parse_flag_tokens(&cli_args);

    let effective = file_flags.union(&cli_flags);
    assert!(effective.watch, "file flags should remain enabled");
    assert!(effective.force_half_cell, "cli flags should be applied");
    assert_eq!(effective.theme, Some(ThemeMode::Dark), "cli should override theme");
    assert_eq!(
        effective.render_debug_log,
        Some(PathBuf::from("file.log")),
        "file config should be preserved when CLI does not override"
    );
}

#[test]
fn test_parse_flag_tokens_handles_equals_syntax() {
    let args = vec![
        "gander".to_string(),
        "--theme=dark".to_string(),
        "--render-debug-log=render.log".to_string(),
    ];
    let flags = parse_flag_tokens(&args);
    assert_eq!(flags.theme, Some(ThemeMode::Dark));
    assert_eq!(flags.render_debug_log, Some(PathBuf::from("render.log")));
}

#[test]
fn test_config_union_merges_booleans() {
    let file = ConfigFlags {
        watch: true,
        no_toc: true,
        ..ConfigFlags::default()
    };
    let cli = ConfigFlags {
        toc: true,
        perf: true,
        ..ConfigFlags::default()
    };
    let merged = file.union(&cli);
    assert!(merged.watch);
    assert!(merged.no_toc);
    assert!(merged.toc);
    assert!(merged.perf);
}
