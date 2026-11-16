//! Pure-Rust PNG collages for Last.fm scrobble charts. No external image tool,
//! no network: the DejaVu font is embedded in the binary so rendering is
//! identical on the developer's Windows box and in the Linux container.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use image::imageops::FilterType;
use image::{Rgba, RgbaImage, imageops};
use std::path::Path;

const FONT: &[u8] = include_bytes!("../../../assets/DejaVuSans.ttf");
const CELL: u32 = 210;
const MARGIN: u32 = 18;
const HEADER: u32 = 88;
const FOOTER: u32 = 56;

const BACKGROUND: Rgba<u8> = Rgba([16, 18, 24, 255]);
const CELL_COLOR: Rgba<u8> = Rgba([34, 38, 48, 255]);
const ACCENT: Rgba<u8> = Rgba([255, 118, 92, 255]);
const TEXT: Rgba<u8> = Rgba([238, 240, 246, 255]);
const MUTED: Rgba<u8> = Rgba([154, 161, 178, 255]);

#[derive(Debug, Clone)]
pub struct CollageItem {
    pub name: String,
    pub secondary: Option<String>,
    pub playcount: u64,
    /// Decoded cover bytes (JPEG/PNG). When absent the cell keeps the plain
    /// dark placeholder.
    pub cover: Option<Vec<u8>>,
}

fn font() -> FontRef<'static> {
    FontRef::try_from_slice(FONT).expect("embedded DejaVuSans.ttf is valid")
}

fn measure(text: &str, size: f32, font: &FontRef<'_>) -> f32 {
    let scaled = font.as_scaled(PxScale::from(size));
    text.chars()
        .map(|ch| scaled.h_advance(font.glyph_id(ch)))
        .sum()
}

fn draw(
    canvas: &mut RgbaImage,
    text: &str,
    x: f32,
    baseline: f32,
    size: f32,
    color: Rgba<u8>,
    font: &FontRef<'_>,
) {
    let scaled = font.as_scaled(PxScale::from(size));
    let mut cursor = x;
    for ch in text.chars() {
        let mut glyph = scaled.scaled_glyph(ch);
        glyph.position = point(cursor, baseline);
        if let Some(outline) = font.outline_glyph(glyph) {
            // draw() yields coordinates relative to the glyph's pixel bounds,
            // not absolute canvas coordinates.
            let origin = outline.px_bounds().min;
            outline.draw(|gx, gy, coverage| {
                let px = origin.x as i32 + gx as i32;
                let py = origin.y as i32 + gy as i32;
                if px >= 0
                    && py >= 0
                    && (px as u32) < canvas.width()
                    && (py as u32) < canvas.height()
                {
                    // Blend onto the opaque canvas: writing a low alpha would
                    // punch transparent holes that viewers render as white.
                    let alpha = (coverage * f32::from(color.0[3]) / 255.0).clamp(0.0, 1.0);
                    let background = canvas.get_pixel(px as u32, py as u32);
                    let mut blended = Rgba([0, 0, 0, 255]);
                    for channel in 0..3 {
                        blended.0[channel] = (f32::from(color.0[channel]) * alpha
                            + f32::from(background.0[channel]) * (1.0 - alpha))
                            .round() as u8;
                    }
                    canvas.put_pixel(px as u32, py as u32, blended);
                }
            });
        }
        cursor += scaled.h_advance(font.glyph_id(ch));
    }
}

fn draw_centered(
    canvas: &mut RgbaImage,
    text: &str,
    center_x: f32,
    baseline: f32,
    size: f32,
    color: Rgba<u8>,
    font: &FontRef<'_>,
) {
    let x = center_x - measure(text, size, font) / 2.0;
    draw(canvas, text, x, baseline, size, color, font);
}

fn fill_rect(canvas: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32, color: Rgba<u8>) {
    for py in y..(y + height).min(canvas.height()) {
        for px in x..(x + width).min(canvas.width()) {
            canvas.put_pixel(px, py, color);
        }
    }
}

fn blend_rect(canvas: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32, alpha: f32) {
    for py in y..(y + height).min(canvas.height()) {
        for px in x..(x + width).min(canvas.width()) {
            let background = canvas.get_pixel(px, py);
            let mut blended = Rgba([0, 0, 0, 255]);
            for channel in 0..3 {
                blended.0[channel] =
                    (f32::from(background.0[channel]) * (1.0 - alpha)).round() as u8;
            }
            canvas.put_pixel(px, py, blended);
        }
    }
}

/// Dark gradient behind the caption so text stays readable over any artwork.
fn shade_bottom(canvas: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32) {
    for row in 0..height {
        let alpha = 0.88 * (row as f32 / height as f32).powi(2);
        blend_rect(canvas, x, y + row, width, 1, alpha);
    }
}

/// Wrap into at most `max_lines`, then truncate the last line with an ellipsis.
fn wrap(text: &str, max_width: f32, max_lines: usize, font: &FontRef<'_>) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if measure(&candidate, 18.0, font) <= max_width {
            current = candidate;
        } else {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            current = word.to_owned();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
    }
    if let Some(last) = lines.last_mut() {
        while measure(last, 18.0, font) > max_width && !last.is_empty() {
            last.pop();
        }
        if !last.is_empty() && measure(&format!("{last}…"), 18.0, font) <= max_width {
            last.push('…');
        }
    }
    lines
}

/// Render a square `grid x grid` collage. Returns a PNG on success.
pub fn render(title: &str, items: &[CollageItem], grid: u8, out: &Path) -> Result<(), String> {
    let grid = grid.clamp(3, 10) as u32;
    let font = font();
    let width = MARGIN * 2 + grid * CELL;
    let height = HEADER + grid * CELL + FOOTER;
    let mut canvas = RgbaImage::from_pixel(width, height, BACKGROUND);

    draw(&mut canvas, title, MARGIN as f32, 46.0, 28.0, TEXT, &font);
    draw(
        &mut canvas,
        "MasloMusicBot · Last.fm",
        MARGIN as f32,
        72.0,
        14.0,
        MUTED,
        &font,
    );

    for index in 0..(grid * grid) as usize {
        let row = index / grid as usize;
        let column = index % grid as usize;
        let x = MARGIN + column as u32 * CELL;
        let y = HEADER + row as u32 * CELL;
        let inner_x = x + 4;
        let inner_y = y + 4;
        let inner_w = CELL - 8;
        let inner_h = CELL - 8;
        fill_rect(&mut canvas, inner_x, inner_y, inner_w, inner_h, CELL_COLOR);

        let center = inner_x as f32 + inner_w as f32 / 2.0;

        let Some(item) = items.get(index) else {
            continue;
        };
        let mut has_cover = false;
        if let Some(bytes) = item.cover.as_deref()
            && let Ok(decoded) = image::load_from_memory(bytes)
        {
            let cover = decoded
                .resize_to_fill(inner_w, inner_h, FilterType::Lanczos3)
                .to_rgba8();
            imageops::overlay(&mut canvas, &cover, i64::from(inner_x), i64::from(inner_y));
            has_cover = true;
            blend_rect(&mut canvas, inner_x, inner_y, 46, 34, 0.62);
            shade_bottom(&mut canvas, inner_x, inner_y + inner_h - 96, inner_w, 96);
        }
        // Rank goes on top of the artwork so it stays visible.
        draw(
            &mut canvas,
            &format!("#{}", index + 1),
            inner_x as f32 + 12.0,
            inner_y as f32 + 26.0,
            16.0,
            ACCENT,
            &font,
        );

        let mut label = item.name.clone();
        if let Some(artist) = &item.secondary {
            if !artist.is_empty() {
                label.push_str(" — ");
                label.push_str(artist);
            }
        }
        let lines = wrap(&label, inner_w as f32 - 24.0, 3, &font);
        let text_top = if has_cover {
            inner_y + inner_h - 92
        } else {
            inner_y + 82
        };
        for (line_index, line) in lines.iter().enumerate() {
            draw_centered(
                &mut canvas,
                line,
                center,
                text_top as f32 + line_index as f32 * 22.0,
                18.0,
                TEXT,
                &font,
            );
        }
        draw_centered(
            &mut canvas,
            &format!("{} plays", item.playcount),
            center,
            inner_y as f32 + inner_h as f32 - 12.0,
            16.0,
            ACCENT,
            &font,
        );
    }

    canvas
        .save(out)
        .map_err(|error| format!("cannot save collage: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_3x3_png() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("collage.png");
        let items: Vec<CollageItem> = (0..9)
            .map(|index| CollageItem {
                name: format!("Track {index}"),
                secondary: Some("Artist".to_owned()),
                playcount: 42 - index,
                cover: None,
            })
            .collect();
        render("Top tracks · 3 months", &items, 3, &path).expect("render");
        let bytes = std::fs::read(&path).expect("png");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
        let decoded = image::load_from_memory(&bytes).expect("decode");
        assert!(decoded.width() > 600 && decoded.height() > 600);
    }

    #[test]
    fn renders_cover_cells_without_panicking() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("with-covers.png");
        let source = RgbaImage::from_pixel(200, 200, Rgba([120, 80, 200, 255]));
        let mut encoded = Vec::new();
        source
            .write_to(
                &mut std::io::Cursor::new(&mut encoded),
                image::ImageFormat::Png,
            )
            .expect("encode cover");
        let items: Vec<CollageItem> = (0..9)
            .map(|index| CollageItem {
                name: format!("Album {index}"),
                secondary: Some("Artist".to_owned()),
                playcount: 100 - index,
                cover: Some(encoded.clone()),
            })
            .collect();
        render("Top albums", &items, 3, &path).expect("render");
        assert!(std::fs::metadata(&path).expect("png").len() > 10_000);
    }
}
