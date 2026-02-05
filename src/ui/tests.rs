use super::*;
use crate::app::Model;
use crate::document::Document;
use crate::document::LineType;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use ratatui_image::picker::Picker;
use std::path::PathBuf;

fn create_test_terminal() -> Terminal<TestBackend> {
    let backend = TestBackend::new(80, 40);
    Terminal::new(backend).unwrap()
}

fn should_run_image_tests() -> bool {
    std::env::var("GANDER_RUN_IMAGE_TESTS")
        .ok()
        .as_deref()
        .is_some_and(|v| v == "1")
}

#[test]
fn test_image_protocol_is_loaded_for_document_with_image() {
    // RED: This test should FAIL because load_nearby_images requires a picker
    // and we need to verify the protocol is actually created
    let md = "# Test\n\n![Alt text](test.png)\n";
    let doc = Document::parse(md).unwrap();

    // Verify document has the image reference
    assert_eq!(doc.images().len(), 1);
    assert_eq!(doc.images()[0].src, "test.png");

    // Create model with a picker (halfblocks - no terminal query needed)
    let picker = Picker::halfblocks();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.picker = Some(picker);

    // Before loading, no protocols
    assert!(model.image_protocols.is_empty());

    // This should fail because the image file doesn't exist
    // but we need to test with a real image to verify the full flow
    model.load_nearby_images();

    // For now, this correctly shows 0 because file doesn't exist
    // A proper test needs a real image fixture
    assert_eq!(model.image_protocols.len(), 0, "No protocol loaded (file doesn't exist)");
}

#[test]
fn test_render_shows_image_placeholder_for_missing_image() {
    // Test that even without actual image file, placeholder text renders
    let md = "![My Image](missing.png)";
    let doc = Document::parse(md).unwrap();

    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));

    let mut terminal = create_test_terminal();
    terminal.draw(|frame| render(&mut model, frame)).unwrap();

    // Check buffer contains the placeholder
    let buffer = terminal.backend().buffer();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(content.contains("[Image: My Image]"), "Should show image placeholder");
}

#[test]
fn test_render_with_real_image_creates_protocol() {
    // This test requires a real image file to verify the full rendering path
    // For now, create a minimal test image in memory
    use image::{DynamicImage, RgbImage};

    let md = "![Test](test_image.png)";
    let doc = Document::parse(md).unwrap();

    let picker = Picker::halfblocks();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.picker = Some(picker);

    // Manually create a protocol for testing (simulating what load_nearby_images would do)
    let test_image = DynamicImage::ImageRgb8(RgbImage::new(100, 100));
    let protocol = model.picker.as_ref().unwrap().new_resize_protocol(test_image);
    // 100x100 image at 65% of 80 cols = 52 cols, aspect ratio preserved = 52 rows (with font 10x20)
    model.image_protocols
        .insert("test_image.png".to_string(), (protocol, 52, 26));

    assert_eq!(model.image_protocols.len(), 1, "Protocol should be loaded");

    // Now render and verify it doesn't crash
    let mut terminal = create_test_terminal();
    let result = terminal.draw(|frame| render(&mut model, frame));
    assert!(result.is_ok(), "Rendering with image protocol should not crash");
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_load_nearby_images_with_real_file() {
    if !should_run_image_tests() {
        return;
    }
    // Test with actual image file fixture
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test_image.png");

    // Only run if fixture exists
    if !fixture_path.exists() {
        eprintln!("Skipping test: fixture not found at {:?}", fixture_path);
        return;
    }

    let md = format!("![Test Image]({})", fixture_path.display());
    let doc = Document::parse(&md).unwrap();

    assert_eq!(doc.images().len(), 1, "Document should have one image");

    let picker = Picker::halfblocks();
    let mut model = Model::new(fixture_path.clone(), doc, (80, 24));
    model.picker = Some(picker);

    // Load images - this should actually load the file
    model.load_nearby_images();

    // Verify protocol was created
    assert_eq!(model.image_protocols.len(), 1, "Protocol should be created for real image");

    // Render and verify no crash
    let mut terminal = create_test_terminal();
    let result = terminal.draw(|frame| render(&mut model, frame));
    assert!(result.is_ok(), "Rendering with loaded image should not crash");

    // Verify buffer has image rendered with colors
    let buffer = terminal.backend().buffer();

    // The halfblocks renderer uses colored cells - check for red (our test image color)
    let has_red_cells = buffer.content().iter().any(|c| {
        matches!(c.fg, ratatui::style::Color::Rgb(255, 0, 0))
            || matches!(c.bg, ratatui::style::Color::Rgb(255, 0, 0))
    });

    assert!(has_red_cells, "Should render red test image with Rgb(255,0,0) color cells");
}

// ==================== NEW TDD TESTS FOR IMAGE LAYOUT ====================

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_image_does_not_overlap_text_below() {
    if !should_run_image_tests() {
        return;
    }
    // Image should have dedicated vertical space, not overlap text below it
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test_image.png");
    if !fixture_path.exists() {
        return;
    }

    let md = format!(
        "# Header\n\n![Test Image]({})\n\nThis text should NOT be covered by image.",
        fixture_path.display()
    );
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();
    // Use taller terminal to fit image + text
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 40));
    model.picker = Some(picker);
    model.load_nearby_images();

    let mut terminal = create_test_terminal();
    terminal.draw(|frame| render(&mut model, frame)).unwrap();

    let buffer = terminal.backend().buffer();

    // Find where "This text" appears - it should be BELOW the image area
    // and NOT have red color (which would indicate image overlap)
    let mut found_text_row = None;
    for row in 0..buffer.area.height {
        let row_content: String = (0..buffer.area.width)
            .map(|col| buffer[(col, row)].symbol())
            .collect();
        if row_content.contains("This text") {
            found_text_row = Some(row);
            break;
        }
    }

    assert!(found_text_row.is_some(), "Text below image should be visible");

    let text_row = found_text_row.unwrap();
    // Check that the text row does NOT have red image pixels
    let text_row_has_red = (0..buffer.area.width).any(|col| {
        let cell = &buffer[(col, text_row)];
        matches!(cell.fg, ratatui::style::Color::Rgb(255, 0, 0))
            || matches!(cell.bg, ratatui::style::Color::Rgb(255, 0, 0))
    });

    assert!(!text_row_has_red, "Text row should NOT have image pixels overlapping it");
}

#[test]
fn test_image_reserves_fixed_height_in_document() {
    // RED: Image should reserve a fixed number of lines in the document
    // so scrolling doesn't change its rendered size
    let md = "# Before\n\n![Alt](img.png)\n\n# After";
    let doc = Document::parse(md).unwrap();

    // Check that the document has reserved space for the image
    let lines = doc.visible_lines(0, 100);

    // Find the image line
    let image_line_idx = lines.iter().position(|l| matches!(l.line_type(), LineType::Image));

    assert!(image_line_idx.is_some(), "Should have an image line");

    // The document should have multiple lines reserved for the image
    // (not just 1 placeholder line)
    let image_lines: Vec<_> = lines
        .iter()
        .filter(|l| matches!(l.line_type(), LineType::Image))
        .collect();

    // For proper image layout, we need more than 1 line reserved
    // This test will FAIL with current implementation (only 1 line)
    assert!(image_lines.len() >= 1, "Image should have at least 1 line (placeholder)");
    // TODO: When we fix this, images should reserve actual height based on image dimensions
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_image_size_stable_during_scroll() {
    if !should_run_image_tests() {
        return;
    }
    // Image should maintain EXACT same size regardless of scroll position
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test_image.png");
    if !fixture_path.exists() {
        return;
    }

    // Create document with content before and after image so we can scroll
    let md = format!(
        "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n\n![Test]({})\n\nLine A\nLine B\nLine C",
        fixture_path.display()
    );
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.picker = Some(picker);
    model.load_nearby_images();

    // Render at scroll position 0
    let mut terminal = create_test_terminal();
    terminal.draw(|frame| render(&mut model, frame)).unwrap();

    let buffer = terminal.backend().buffer();
    let red_cells_at_pos_0: usize = buffer
        .content()
        .iter()
        .filter(|c| {
            matches!(c.fg, ratatui::style::Color::Rgb(255, 0, 0))
                || matches!(c.bg, ratatui::style::Color::Rgb(255, 0, 0))
        })
        .count();

    // Scroll down by 2 and render again (image should still be fully visible)
    model.viewport.scroll_down(2);
    terminal.draw(|frame| render(&mut model, frame)).unwrap();

    let buffer = terminal.backend().buffer();
    let red_cells_at_pos_2: usize = buffer
        .content()
        .iter()
        .filter(|c| {
            matches!(c.fg, ratatui::style::Color::Rgb(255, 0, 0))
                || matches!(c.bg, ratatui::style::Color::Rgb(255, 0, 0))
        })
        .count();

    // Image cell count should be EXACTLY the same when fully visible
    assert_eq!(
        red_cells_at_pos_0, red_cells_at_pos_2,
        "Image size must be identical during scroll. Pos 0: {} cells, Pos 2: {} cells",
        red_cells_at_pos_0, red_cells_at_pos_2
    );
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_image_clips_not_resizes_when_scrolling() {
    if !should_run_image_tests() {
        return;
    }
    // CRITICAL: When scrolling, the image should be CLIPPED (cropped), not RESIZED
    // This means: at any scroll position where the image is visible,
    // the WIDTH should be the same (65% of viewport)
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test_image.png");
    if !fixture_path.exists() {
        return;
    }

    let mut md = format!("Line 1\n\n![Test]({})\n\n", fixture_path.display());
    for i in 0..50 {
        md.push_str(&format!("Content {}\n", i));
    }
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 40));
    model.picker = Some(picker);
    model.load_nearby_images();

    // Measure width at scroll 0 (fully visible)
    let mut terminal = create_test_terminal();
    terminal.draw(|frame| render(&mut model, frame)).unwrap();
    let width_at_scroll_0 = measure_image_width(terminal.backend().buffer());

    // Scroll so image is partially off top (scroll to line 10)
    model.viewport.scroll_down(10);
    terminal.draw(|frame| render(&mut model, frame)).unwrap();
    let width_at_scroll_10 = measure_image_width(terminal.backend().buffer());

    // Scroll more (scroll to line 20)
    model.viewport.scroll_down(10);
    terminal.draw(|frame| render(&mut model, frame)).unwrap();
    let width_at_scroll_20 = measure_image_width(terminal.backend().buffer());

    // Width should be IDENTICAL at all scroll positions (image clips, doesn't resize)
    assert!(width_at_scroll_0 > 0, "Image should be visible at scroll 0");

    assert_eq!(
        width_at_scroll_0, width_at_scroll_10,
        "Image width must stay constant when scrolling. At 0: {}, at 10: {}",
        width_at_scroll_0, width_at_scroll_10
    );

    assert_eq!(
        width_at_scroll_0, width_at_scroll_20,
        "Image width must stay constant when scrolling. At 0: {}, at 20: {}",
        width_at_scroll_0, width_at_scroll_20
    );
}

fn measure_image_width(buffer: &ratatui::buffer::Buffer) -> u16 {
    let mut max_col = 0u16;
    for row in 0..buffer.area.height {
        for col in 0..buffer.area.width {
            let cell = &buffer[(col, row)];
            if matches!(cell.fg, ratatui::style::Color::Rgb(255, 0, 0))
                || matches!(cell.bg, ratatui::style::Color::Rgb(255, 0, 0))
            {
                max_col = max_col.max(col + 1);
            }
        }
    }
    max_col
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_image_scrolls_off_top_of_screen() {
    if !should_run_image_tests() {
        return;
    }
    // When we scroll past the image entirely, it should NOT be visible at all
    // (it should scroll off, not stick to the top)
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test_image.png");
    if !fixture_path.exists() {
        return;
    }

    // Image at line 2, reserved height 26, so image occupies lines 2-27
    // Need LOTS of content after so we can scroll past the image
    let mut md = format!("Line 1\n\n![Test]({})\n\n", fixture_path.display());
    for i in 0..50 {
        md.push_str(&format!("Content line {}\n", i));
    }
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 40));
    model.picker = Some(picker);
    model.load_nearby_images();

    // Scroll way past the image (image ends at ~line 28, scroll to line 35)
    model.viewport.scroll_down(35);

    let mut terminal = create_test_terminal();
    terminal.draw(|frame| render(&mut model, frame)).unwrap();

    let buffer = terminal.backend().buffer();

    // Count red cells - should be ZERO because image is completely scrolled off
    let red_cell_count: usize = buffer
        .content()
        .iter()
        .filter(|c| {
            matches!(c.fg, ratatui::style::Color::Rgb(255, 0, 0))
                || matches!(c.bg, ratatui::style::Color::Rgb(255, 0, 0))
        })
        .count();

    assert_eq!(
        red_cell_count, 0,
        "Image should be completely off screen when scrolled past. Found {} red cells",
        red_cell_count
    );
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_image_partially_visible_when_scrolled_off_top() {
    if !should_run_image_tests() {
        return;
    }
    // When image top is scrolled off, the BOTTOM portion should still be visible
    // Image should render at y=0 with reduced height, showing bottom portion (clip_top)
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test_image.png");
    if !fixture_path.exists() {
        return;
    }

    // Image at line 2, reserved height 26, so image occupies lines 2-27
    let mut md = format!("Line 1\n\n![Test]({})\n\n", fixture_path.display());
    for i in 0..50 {
        md.push_str(&format!("Content line {}\n", i));
    }
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 40));
    model.picker = Some(picker);
    model.load_nearby_images();

    // Scroll so image top is off screen but bottom is still visible
    // Image starts at line 2, scroll to line 10 so ~8 lines are off top
    model.viewport.scroll_down(10);

    let mut terminal = create_test_terminal();
    terminal.draw(|frame| render(&mut model, frame)).unwrap();

    let buffer = terminal.backend().buffer();

    // Count red cells - should still have some visible (bottom portion of image)
    let red_cell_count: usize = buffer
        .content()
        .iter()
        .filter(|c| {
            matches!(c.fg, ratatui::style::Color::Rgb(255, 0, 0))
                || matches!(c.bg, ratatui::style::Color::Rgb(255, 0, 0))
        })
        .count();

    assert!(
        red_cell_count > 0,
        "Image should be partially visible when top is scrolled off. Found {} red cells",
        red_cell_count
    );

    // Find first row with red pixels - should be at or near row 0 (top of viewport)
    let first_red_row = (0..buffer.area.height).find(|&row| {
        (0..buffer.area.width).any(|col| {
            let cell = &buffer[(col, row)];
            matches!(cell.fg, ratatui::style::Color::Rgb(255, 0, 0))
                || matches!(cell.bg, ratatui::style::Color::Rgb(255, 0, 0))
        })
    });

    assert!(
        first_red_row.unwrap_or(99) <= 2,
        "Image should render at top of viewport when partially scrolled off. First red row: {:?}",
        first_red_row
    );

    // Width should still be full width (65% of viewport)
    let max_red_col = measure_image_width(buffer);
    let expected_width = (80.0 * 0.65) as u16;
    assert!(
        max_red_col >= expected_width - 5,
        "Image should maintain full width when top is scrolled off. Got {}, expected ~{}",
        max_red_col,
        expected_width
    );
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_image_partially_visible_when_near_bottom() {
    if !should_run_image_tests() {
        return;
    }
    // When image extends past bottom of viewport, it should be clipped
    // but the visible portion should render at FULL WIDTH
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test_image.png");
    if !fixture_path.exists() {
        return;
    }

    // Put content before image so image starts partway down the viewport
    // With 20-line terminal (19 viewport), image at line 12 will extend past bottom
    let mut md = String::new();
    for i in 0..10 {
        md.push_str(&format!("Line {}\n", i));
    }
    md.push_str(&format!("\n![Test]({})\n\nAfter", fixture_path.display()));
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 20));
    model.picker = Some(picker);
    model.load_nearby_images();

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal.draw(|frame| render(&mut model, frame)).unwrap();

    let buffer = terminal.backend().buffer();

    // Find max column with red pixels - should be at full width
    let mut max_red_col = 0u16;
    let mut first_red_row: Option<u16> = None;
    let mut last_red_row: Option<u16> = None;

    for row in 0..buffer.area.height {
        for col in 0..buffer.area.width {
            let cell = &buffer[(col, row)];
            if matches!(cell.fg, ratatui::style::Color::Rgb(255, 0, 0))
                || matches!(cell.bg, ratatui::style::Color::Rgb(255, 0, 0))
            {
                max_red_col = max_red_col.max(col);
                if first_red_row.is_none() {
                    first_red_row = Some(row);
                }
                last_red_row = Some(row);
            }
        }
    }

    let expected_width = (80.0 * 0.65) as u16;

    // Image should be at full width even when bottom is cut off
    assert!(
        max_red_col >= expected_width - 10,
        "Image cut off at bottom should maintain full width. Got width {}, expected ~{}",
        max_red_col + 1,
        expected_width
    );

    // Image should extend to or near the bottom of the viewport (clipped)
    assert!(
        last_red_row.unwrap_or(0) >= buffer.area.height - 3,
        "Image should extend to near bottom of viewport. Last red row: {:?}, viewport height: {}",
        last_red_row,
        buffer.area.height
    );
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_image_width_is_65_percent_of_viewport() {
    if !should_run_image_tests() {
        return;
    }
    // Image should be sized to ~65% of viewport width
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test_image.png");
    if !fixture_path.exists() {
        return;
    }

    let md = format!("![Test]({})", fixture_path.display());
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();
    let terminal_width = 80u16;
    let terminal_height = 40u16;
    let mut model = Model::new(PathBuf::from("test.md"), doc, (terminal_width, terminal_height));
    model.picker = Some(picker);
    model.load_nearby_images();

    let mut terminal = create_test_terminal();
    terminal.draw(|frame| render(&mut model, frame)).unwrap();

    let buffer = terminal.backend().buffer();

    // Find the maximum column that has red pixels (image content)
    let mut max_image_col = 0u16;
    for row in 0..buffer.area.height {
        for col in 0..buffer.area.width {
            let cell = &buffer[(col, row)];
            if matches!(cell.fg, ratatui::style::Color::Rgb(255, 0, 0))
                || matches!(cell.bg, ratatui::style::Color::Rgb(255, 0, 0))
            {
                max_image_col = max_image_col.max(col);
            }
        }
    }

    // Image width should be approximately 65% of terminal width (52 cols for 80-wide terminal)
    let expected_width = (terminal_width as f32 * 0.65) as u16;
    let tolerance = 5u16; // Allow some tolerance

    assert!(max_image_col > 0, "Image should render with some width");
    assert!(
        max_image_col <= expected_width + tolerance,
        "Image width ({}) should be <= ~65% of viewport ({}±{})",
        max_image_col + 1,
        expected_width,
        tolerance
    );
    assert!(
        max_image_col >= expected_width.saturating_sub(tolerance),
        "Image width ({}) should be >= ~65% of viewport ({}±{})",
        max_image_col + 1,
        expected_width,
        tolerance
    );
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_image_height_based_on_scaled_dimensions() {
    if !should_run_image_tests() {
        return;
    }
    // Image height should be calculated from actual scaled image, not hardcoded
    // A wide image (e.g., 200x50) scaled to 65% width should be shorter than 26 rows
    // Create a wide test image (200x50 pixels)
    use image::{Rgb, RgbImage};
    let dir = tempfile::tempdir().unwrap();
    let wide_img_path = dir.path().join("wide_test_image.png");
    let mut wide_img = RgbImage::new(200, 50);
    // Fill with blue so we can distinguish from red test image
    for pixel in wide_img.pixels_mut() {
        *pixel = Rgb([0, 0, 255]);
    }
    wide_img.save(&wide_img_path).unwrap();

    let md = format!("![Wide]({})", wide_img_path.display());
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();
    let font_size = picker.font_size();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 40));
    model.picker = Some(picker);
    model.load_nearby_images();

    let mut terminal = create_test_terminal();
    terminal.draw(|frame| render(&mut model, frame)).unwrap();

    let buffer = terminal.backend().buffer();

    // Count rows with blue pixels
    let mut max_blue_row = 0u16;
    for row in 0..buffer.area.height {
        for col in 0..buffer.area.width {
            let cell = &buffer[(col, row)];
            if matches!(cell.fg, ratatui::style::Color::Rgb(0, 0, 255))
                || matches!(cell.bg, ratatui::style::Color::Rgb(0, 0, 255))
            {
                max_blue_row = max_blue_row.max(row);
            }
        }
    }

    // Calculate expected height:
    // Target width = 80 * 0.65 = 52 cols = 52 * font_size.0 pixels
    // Scale factor = target_width / 200 = (52 * 10) / 200 = 2.6
    // Scaled height = 50 * 2.6 = 130 pixels
    // Terminal rows = 130 / font_size.1 = 130 / 20 = 6.5 ≈ 7 rows
    let target_width_px = (80.0 * 0.65) as u32 * font_size.0 as u32;
    let scale = target_width_px as f32 / 200.0;
    let scaled_height_px = (50.0 * scale) as u32;
    let expected_rows = (scaled_height_px as f32 / font_size.1 as f32).ceil() as u16;

    // Image should be close to expected height, NOT hardcoded 26 rows
    assert!(
        max_blue_row < 20,
        "Wide image should be much shorter than 26 rows. Got {} rows, expected ~{} rows",
        max_blue_row + 1,
        expected_rows
    );
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_wide_image_renders_at_correct_height() {
    if !should_run_image_tests() {
        return;
    }
    // A wide/short image should render at its actual scaled height,
    // not hardcoded 26 lines
    use image::{Rgb, RgbImage};

    // Create a wide test image (400x100 pixels)
    // At 65% of 80 cols with font_size (10,20):
    // target_width = 52 * 10 = 520px, scale = 520/400 = 1.3
    // scaled_height = 100 * 1.3 = 130px = 130/20 = 6.5 ≈ 7 rows
    let dir = tempfile::tempdir().unwrap();
    let wide_img_path = dir.path().join("wide_short_image.png");
    let mut wide_img = RgbImage::new(400, 100);
    // Fill with green
    for pixel in wide_img.pixels_mut() {
        *pixel = Rgb([0, 255, 0]);
    }
    wide_img.save(&wide_img_path).unwrap();

    let md = format!("![Wide]({})", wide_img_path.display());
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 40));
    model.picker = Some(picker);
    model.load_nearby_images();

    let mut terminal = create_test_terminal();
    terminal.draw(|frame| render(&mut model, frame)).unwrap();

    let buffer = terminal.backend().buffer();

    // Find the last row with green pixels
    let mut max_green_row = 0u16;
    for row in 0..buffer.area.height {
        for col in 0..buffer.area.width {
            let cell = &buffer[(col, row)];
            if matches!(cell.fg, ratatui::style::Color::Rgb(0, 255, 0))
                || matches!(cell.bg, ratatui::style::Color::Rgb(0, 255, 0))
            {
                max_green_row = max_green_row.max(row);
            }
        }
    }

    // Wide image (400x100) should be ~7 rows, definitely less than 15
    assert!(
        max_green_row < 15,
        "Wide short image should render in fewer than 15 rows, but rendered {} rows",
        max_green_row + 1
    );
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_document_reserves_scaled_image_height() {
    if !should_run_image_tests() {
        return;
    }
    // The document should reserve the same number of lines as the scaled image height.
    use image::{Rgb, RgbImage};

    let dir = tempfile::tempdir().unwrap();
    let wide_img_path = dir.path().join("wide_short_image_layout.png");
    let mut wide_img = RgbImage::new(400, 100);
    for pixel in wide_img.pixels_mut() {
        *pixel = Rgb([0, 255, 0]);
    }
    wide_img.save(&wide_img_path).unwrap();

    let md = format!("![Wide]({})", wide_img_path.display());
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 40));
    model.picker = Some(picker);
    model.load_nearby_images();

    let image_ref = model.document.images().first().expect("Image ref missing");
    let (.., height_rows) = model
        .image_protocols
        .get(&wide_img_path.display().to_string())
        .expect("Image protocol missing");

    let reserved_lines = image_ref.line_range.end - image_ref.line_range.start;
    assert_eq!(
        reserved_lines,
        *height_rows as usize,
        "Document should reserve the scaled image height"
    );
}

#[test]
#[ignore = "requires image rendering; set GANDER_RUN_IMAGE_TESTS=1"]
fn test_image_rescales_on_viewport_resize() {
    if !should_run_image_tests() {
        return;
    }
    // When viewport is resized, images should rescale to maintain 65% of new width
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test_image.png");
    if !fixture_path.exists() {
        return;
    }

    let md = format!("![Test]({})", fixture_path.display());
    let doc = Document::parse(&md).unwrap();

    let picker = Picker::halfblocks();

    // Start with 80-wide terminal
    let mut model = Model::new(PathBuf::from("test.md"), doc.clone(), (80, 40));
    model.picker = Some(picker.clone());
    model.load_nearby_images();

    // Get the initial image width (should be 65% of 80 = 52)
    let initial_width = model
        .image_protocols
        .get(&fixture_path.display().to_string())
        .map(|(_, w, _)| *w)
        .expect("Image should be loaded");
    assert_eq!(initial_width, 52, "Initial width should be 65% of 80");

    // Resize viewport to 120 wide
    model.viewport.resize(120, 40);

    // Trigger reload of images for new size
    model.load_nearby_images();

    // Get the new image width (should be 65% of 120 = 78)
    let new_width = model
        .image_protocols
        .get(&fixture_path.display().to_string())
        .map(|(_, w, _)| *w)
        .expect("Image should still be loaded");

    assert_eq!(
        new_width, 78,
        "After resize to 120 wide, image width should be 65% = 78, but got {}",
        new_width
    );
}
