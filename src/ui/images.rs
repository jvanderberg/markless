use ratatui::prelude::*;
use ratatui_image::protocol::StatefulProtocolType;
use ratatui_image::{CropOptions, Resize, StatefulImage};

use crate::app::Model;

pub fn render_images(model: &mut Model, frame: &mut Frame, doc_area: Rect) {
    // Render images to temp buffer, copy visible portion to frame
    let vp_top = model.viewport.offset() as i32;
    let vp_bottom = vp_top + doc_area.height as i32;
    let image_scroll_settling = model.is_image_scroll_settling();
    crate::perf::log_event(
        "render.document",
        format!(
            "vp={}..{} doc_area={}x{} images_cached={}",
            vp_top,
            vp_bottom,
            doc_area.width,
            doc_area.height,
            model.image_protocols.len()
        ),
    );

    if model.image_protocols.is_empty() {
        return;
    }

    for img_ref in model.document.images() {
        let Some((protocol, img_width, img_height)) = model.image_protocols.get_mut(&img_ref.src)
        else {
            continue;
        };
        let img_width = *img_width;
        let img_height = *img_height;

        let img_top = img_ref.line_range.start as i32;
        let img_bottom = img_top + img_height as i32;

        // Skip if no overlap with viewport
        if img_bottom <= vp_top || img_top >= vp_bottom {
            continue;
        }
        crate::perf::log_event(
            "render.image.visible",
            format!(
                "src={} img={}..{} img_size={}x{}",
                img_ref.src, img_top, img_bottom, img_width, img_height
            ),
        );

        // Calculate which rows of temp buffer are visible
        let rel_y = img_top - vp_top;
        let src_start = (-rel_y).max(0) as u16;
        let dst_y = doc_area.y + rel_y.max(0) as u16;
        let visible_rows = (img_bottom.min(vp_bottom) - img_top.max(vp_top)) as u16;
        let visible_cols = img_width.min(doc_area.width);
        if visible_rows == 0 || visible_cols == 0 {
            continue;
        }

        if matches!(protocol.protocol_type(), StatefulProtocolType::ITerm2(_)) {
            if image_scroll_settling {
                // iTerm2/Warp can flicker when re-embedding inline images during rapid scroll.
                // Draw a cheap gray placeholder while scrolling; restore image on settle.
                let frame_buf = frame.buffer_mut();
                for row in 0..visible_rows {
                    let dst_row = dst_y + row;
                    if dst_row >= frame_buf.area.height {
                        continue;
                    }
                    for col in 0..visible_cols {
                        let dst_cell = &mut frame_buf[(doc_area.x + col, dst_row)];
                        dst_cell
                            .set_symbol(" ")
                            .set_bg(Color::DarkGray)
                            .set_fg(Color::DarkGray)
                            .set_skip(false);
                    }
                }
                crate::perf::log_event(
                    "render.image.placeholder",
                    format!(
                        "src={} y={} rows={} cols={} reason=active-scroll",
                        img_ref.src, dst_y, visible_rows, visible_cols
                    ),
                );
                continue;
            }

            // iTerm2 inline graphics store the payload in a single anchor cell; row-slicing the
            // rendered buffer breaks scrolling and causes stale/overwritten content.
            let crop = if src_start > 0 {
                Resize::Crop(Some(CropOptions {
                    clip_top: true,
                    clip_left: false,
                }))
            } else {
                Resize::Crop(None)
            };
            let image_widget = StatefulImage::default().resize(crop);
            image_widget.render(
                Rect::new(doc_area.x, dst_y, visible_cols, visible_rows),
                frame.buffer_mut(),
                protocol,
            );
            crate::perf::log_event(
                "render.image.direct",
                format!(
                    "src={} y={} rows={} cols={} src_start={} mode=iterm2-crop",
                    img_ref.src, dst_y, visible_rows, visible_cols, src_start
                ),
            );
            continue;
        }

        // Non-iTerm2 protocols are safe to render to a temp buffer and then blit row slices.
        let temp_area = Rect::new(0, 0, img_width, img_height);
        let mut temp_buf = ratatui::buffer::Buffer::empty(temp_area);
        let resize = if matches!(protocol.protocol_type(), StatefulProtocolType::Halfblocks(_)) {
            // Nearest-neighbor causes strong color aliasing artifacts in half-cell mode.
            Resize::Scale(Some(image::imageops::FilterType::CatmullRom))
        } else {
            Resize::Scale(None)
        };
        let image_widget = StatefulImage::default().resize(resize);
        image_widget.render(temp_area, &mut temp_buf, protocol);

        // Terminal.app and other non-truecolor terminals behave better with indexed colors
        // than repeated truecolor updates in halfblock mode.
        if matches!(protocol.protocol_type(), StatefulProtocolType::Halfblocks(_))
            && !crate::image::supports_truecolor_terminal()
        {
            for row in 0..temp_area.height {
                for col in 0..temp_area.width {
                    let cell = &mut temp_buf[(col, row)];
                    if let Color::Rgb(r, g, b) = cell.fg {
                        cell.fg = Color::Indexed(rgb_to_xterm_256(r, g, b));
                    }
                    if let Color::Rgb(r, g, b) = cell.bg {
                        cell.bg = Color::Indexed(rgb_to_xterm_256(r, g, b));
                    }
                }
            }
        }

        // Copy visible rows from temp buffer to frame buffer
        let frame_buf = frame.buffer_mut();
        for row in 0..visible_rows {
            let src_row = src_start + row;
            let dst_row = dst_y + row;
            if src_row < img_height && dst_row < frame_buf.area.height {
                for col in 0..visible_cols {
                    let src_cell = &temp_buf[(col, src_row)];
                    let dst_cell = &mut frame_buf[(doc_area.x + col, dst_row)];
                    *dst_cell = src_cell.clone();
                }
            }
        }
        crate::perf::log_event(
            "render.image.blit",
            format!(
                "src={} src_start={} dst_y={} rows={} cols={}",
                img_ref.src,
                src_start,
                dst_y,
                visible_rows,
                visible_cols
            ),
        );
    }
}

fn rgb_to_xterm_256(r: u8, g: u8, b: u8) -> u8 {
    let to_cube = |v: u8| ((v as u16 * 5) / 255) as u8;
    let ri = to_cube(r);
    let gi = to_cube(g);
    let bi = to_cube(b);
    16 + (36 * ri) + (6 * gi) + bi
}
