//! Pure-Rust PNG collages for Last.fm scrobble charts. No external image tool,
//! no network: the DejaVu font is embedded in the binary so rendering is
//! identical on the developer's Windows box and in the Linux container.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

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
    /// Cover bytes (JPEG/PNG) shared between cells; duplicate artwork is one
    /// allocation. When absent the cell keeps the plain dark placeholder.
    pub cover: Option<Arc<[u8]>>,
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
    let x_end = (x + width).min(canvas.width());
    let y_end = (y + height).min(canvas.height());
    if x >= x_end || y >= y_end {
        return;
    }
    let row_bytes = (x_end - x) as usize * 4;
    let stride = canvas.width() as usize * 4;
    // Build the row once, then memcpy it per scanline; per-pixel writes meant
    // three bounds checks and a `put_pixel` call each.
    let mut line = vec![0u8; row_bytes];
    for pixel in line.as_chunks_mut::<4>().0 {
        pixel.copy_from_slice(&color.0);
    }
    let data = canvas.as_mut();
    for py in y..y_end {
        let start = py as usize * stride + x as usize * 4;
        data[start..start + row_bytes].copy_from_slice(&line);
    }
}

fn blend_rect(canvas: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32, alpha: f32) {
    let x_end = (x + width).min(canvas.width());
    let y_end = (y + height).min(canvas.height());
    if x >= x_end || y >= y_end {
        return;
    }
    let factor = (1.0 - alpha).clamp(0.0, 1.0);
    // 256-entry lookup instead of a float multiply per channel per pixel; the
    // bottom shade alone darkens ~20 000 pixels per cover cell.
    let mut lut = [0u8; 256];
    for (value, slot) in lut.iter_mut().enumerate() {
        *slot = (value as f32 * factor).round() as u8;
    }
    let row_bytes = (x_end - x) as usize * 4;
    let stride = canvas.width() as usize * 4;
    let data = canvas.as_mut();
    for py in y..y_end {
        let start = py as usize * stride + x as usize * 4;
        for pixel in data[start..start + row_bytes].as_chunks_mut::<4>().0 {
            pixel[0] = lut[pixel[0] as usize];
            pixel[1] = lut[pixel[1] as usize];
            pixel[2] = lut[pixel[2] as usize];
            // Keep the canvas opaque: transparent PNG covers would otherwise
            // leave holes that viewers render as white.
            pixel[3] = 255;
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
fn wrap(
    text: &str,
    max_width: f32,
    max_lines: usize,
    size: f32,
    font: &FontRef<'_>,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if measure(&candidate, size, font) <= max_width {
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
        while measure(last, size, font) > max_width && !last.is_empty() {
            last.pop();
        }
        if !last.is_empty() && measure(&format!("{last}…"), size, font) <= max_width {
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

    // Album/artist collages show the same artwork in several cells; decode and
    // resize every unique cover once per render (keyed by the shared `Arc`).
    let mut covers: HashMap<usize, Option<RgbaImage>> = HashMap::new();
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
        let cover_ptr = item
            .cover
            .as_ref()
            .map(|bytes| Arc::as_ptr(bytes) as *const u8 as usize);
        if let Some(pointer) = cover_ptr {
            covers.entry(pointer).or_insert_with(|| {
                item.cover
                    .as_deref()
                    .and_then(|bytes| image::load_from_memory(bytes).ok())
                    // Triangle is several times cheaper than Lanczos3 at this
                    // scale; the 202 px cell makes the difference invisible.
                    .map(|decoded| {
                        decoded
                            .resize_to_fill(inner_w, inner_h, FilterType::Triangle)
                            .to_rgba8()
                    })
            });
        }
        if let Some(cover) = cover_ptr
            .and_then(|pointer| covers.get(&pointer))
            .and_then(Option::as_ref)
        {
            imageops::overlay(&mut canvas, cover, i64::from(inner_x), i64::from(inner_y));
            has_cover = true;
            shade_bottom(&mut canvas, inner_x, inner_y + inner_h - 96, inner_w, 96);
        }
        let mut label = item.name.clone();
        if let Some(artist) = &item.secondary
            && !artist.is_empty()
        {
            label.push_str(" — ");
            label.push_str(artist);
        }
        let lines = wrap(&label, inner_w as f32 - 24.0, 3, 18.0, &font);
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

/// One column of the genre histogram.
#[derive(Debug, Clone)]
pub struct BarItem {
    pub label: String,
    pub value: u64,
}

/// Render a vertical bar chart (the genre analyzer). `items` are expected to be
/// already sorted by value, highest first. The title/subtitle sit in the header;
/// each bar carries its value and a wrapped label underneath.
pub fn render_bars(
    title: &str,
    subtitle: &str,
    items: &[BarItem],
    out: &Path,
) -> Result<(), String> {
    if items.is_empty() {
        return Err("no bars to render".to_owned());
    }
    let font = font();
    let bars = items.len() as u32;
    const HEADER_H: u32 = 116;
    const PLOT_H: u32 = 400;
    const LABEL_H: u32 = 150;
    const FOOTER_H: u32 = 34;
    let width = (MARGIN * 2 + bars * 128).clamp(1_000, 2_400);
    let height = HEADER_H + PLOT_H + LABEL_H + FOOTER_H;
    let slot = (width - MARGIN * 2) / bars;
    let bar_width = (slot as f32 * 0.62).clamp(28.0, 110.0) as u32;
    let plot_bottom = HEADER_H + PLOT_H;
    let max_value = items
        .iter()
        .map(|item| item.value)
        .max()
        .unwrap_or(1)
        .max(1);
    let mut canvas = RgbaImage::from_pixel(width, height, BACKGROUND);

    draw(&mut canvas, title, MARGIN as f32, 48.0, 30.0, TEXT, &font);
    draw(
        &mut canvas,
        subtitle,
        MARGIN as f32,
        82.0,
        15.0,
        MUTED,
        &font,
    );
    fill_rect(
        &mut canvas,
        MARGIN,
        plot_bottom,
        width - MARGIN * 2,
        1,
        Rgba([70, 76, 92, 255]),
    );

    for (index, item) in items.iter().enumerate() {
        let center = MARGIN as f32 + slot as f32 * index as f32 + slot as f32 / 2.0;
        let bar_height = (((PLOT_H - 42) as u64 * item.value / max_value).max(3)) as u32;
        let bar_top = plot_bottom - bar_height;
        let bar_left = center as u32 - bar_width / 2;
        fill_rect(
            &mut canvas,
            bar_left,
            bar_top,
            bar_width,
            bar_height,
            ACCENT,
        );
        draw_centered(
            &mut canvas,
            &item.value.to_string(),
            center,
            bar_top as f32 - 10.0,
            15.0,
            TEXT,
            &font,
        );
        let lines = wrap(&item.label, slot as f32 - 10.0, 3, 13.0, &font);
        for (line_index, line) in lines.iter().enumerate() {
            draw_centered(
                &mut canvas,
                line,
                center,
                plot_bottom as f32 + 24.0 + line_index as f32 * 16.0,
                13.0,
                MUTED,
                &font,
            );
        }
    }

    draw(
        &mut canvas,
        "MasloMusicBot · genre analyzer",
        MARGIN as f32,
        height as f32 - 12.0,
        12.0,
        MUTED,
        &font,
    );
    canvas
        .save(out)
        .map_err(|error| format!("cannot save chart: {error}"))
}

/// RGB triplet for one dynamics series. Kept dependency-free so callers do not
/// need the `image` crate just to pick a palette entry.
#[derive(Debug, Clone, Copy)]
pub struct SeriesColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Eight distinguishable line colors for the genre dynamics chart.
pub const SERIES_COLORS: [SeriesColor; 8] = [
    SeriesColor {
        r: 255,
        g: 118,
        b: 92,
    },
    SeriesColor {
        r: 94,
        g: 200,
        b: 255,
    },
    SeriesColor {
        r: 154,
        g: 232,
        b: 124,
    },
    SeriesColor {
        r: 255,
        g: 205,
        b: 92,
    },
    SeriesColor {
        r: 194,
        g: 150,
        b: 255,
    },
    SeriesColor {
        r: 255,
        g: 140,
        b: 200,
    },
    SeriesColor {
        r: 110,
        g: 220,
        b: 200,
    },
    SeriesColor {
        r: 235,
        g: 160,
        b: 110,
    },
];

/// One genre line. `values` is aligned with the `labels` slice passed to
/// [`render_lines`]; a shorter vector is padded with zeroes.
#[derive(Debug, Clone)]
pub struct LineSeries {
    pub label: String,
    pub color: SeriesColor,
    pub values: Vec<u64>,
}

fn draw_segment(canvas: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = dx.abs().max(dy.abs()).max(1);
    for step in 0..=steps {
        let x = x0 + (dx * step) / steps;
        let y = y0 + (dy * step) / steps;
        fill_rect(canvas, x.max(0) as u32, y.max(0) as u32, 3, 3, color);
    }
}

/// Render a multi-line time chart (the "genre dynamics" graph). `labels` are
/// short bucket captions (e.g. `11-18`) on the x axis; each series carries one
/// value per label. Pure Rust: same embedded font, no network, no image tools.
pub fn render_lines(
    title: &str,
    subtitle: &str,
    labels: &[String],
    series: &[LineSeries],
    out: &Path,
) -> Result<(), String> {
    if labels.is_empty() || series.is_empty() {
        return Err("no lines to render".to_owned());
    }
    let font = font();
    const WIDTH: u32 = 1_200;
    const HEIGHT: u32 = 720;
    const PLOT_TOP: u32 = 140;
    const PLOT_BOTTOM: u32 = 580;
    const PLOT_LEFT: u32 = 96;
    const PLOT_RIGHT: u32 = 1_150;
    let mut canvas = RgbaImage::from_pixel(WIDTH, HEIGHT, BACKGROUND);

    draw(&mut canvas, title, MARGIN as f32, 48.0, 30.0, TEXT, &font);
    draw(
        &mut canvas,
        subtitle,
        MARGIN as f32,
        84.0,
        15.0,
        MUTED,
        &font,
    );

    let max_value = series
        .iter()
        .flat_map(|line| line.values.iter().copied())
        .max()
        .unwrap_or(1)
        .max(1);
    let grid = Rgba([64, 70, 86, 255]);
    for step in 0..=4u32 {
        let y = PLOT_TOP + (PLOT_BOTTOM - PLOT_TOP) * step / 4;
        fill_rect(&mut canvas, PLOT_LEFT, y, PLOT_RIGHT - PLOT_LEFT, 1, grid);
        let value = max_value * u64::from(4 - step) / 4;
        draw(
            &mut canvas,
            &value.to_string(),
            PLOT_LEFT as f32 - 12.0 - measure(&value.to_string(), 13.0, &font),
            y as f32 - 8.0,
            13.0,
            MUTED,
            &font,
        );
    }
    fill_rect(
        &mut canvas,
        PLOT_LEFT,
        PLOT_TOP,
        1,
        PLOT_BOTTOM - PLOT_TOP,
        grid,
    );

    let count = labels.len();
    let slot = (PLOT_RIGHT - PLOT_LEFT) / count as u32;
    let skip = if count > 12 { count.div_ceil(12) } else { 1 };
    for (index, label) in labels.iter().enumerate() {
        if index % skip != 0 && index + 1 != count {
            continue;
        }
        let center = PLOT_LEFT as f32 + slot as f32 * index as f32 + slot as f32 / 2.0;
        draw_centered(
            &mut canvas,
            label,
            center,
            PLOT_BOTTOM as f32 + 26.0,
            13.0,
            MUTED,
            &font,
        );
    }

    for line in series {
        let color = Rgba([line.color.r, line.color.g, line.color.b, 255]);
        let mut previous: Option<(i32, i32)> = None;
        for index in 0..count {
            let value = line.values.get(index).copied().unwrap_or(0);
            let center = PLOT_LEFT as f32 + slot as f32 * index as f32 + slot as f32 / 2.0;
            let y = PLOT_BOTTOM - ((PLOT_BOTTOM - PLOT_TOP) as u64 * value / max_value) as u32;
            let point = (center as i32, y as i32);
            if let Some((x0, y0)) = previous {
                draw_segment(&mut canvas, x0, y0, point.0, point.1, color);
            }
            previous = Some(point);
        }
    }

    // Legend: two columns of color swatches under the plot.
    let columns = 2;
    let rows = series.len().div_ceil(columns);
    for (index, line) in series.iter().enumerate() {
        let column = index / rows;
        let row = index % rows;
        let x = MARGIN + column as u32 * 560;
        let y = 636 + row as u32 * 26;
        fill_rect(
            &mut canvas,
            x,
            y - 11,
            14,
            14,
            Rgba([line.color.r, line.color.g, line.color.b, 255]),
        );
        draw(
            &mut canvas,
            &line.label,
            x as f32 + 22.0,
            y as f32,
            15.0,
            TEXT,
            &font,
        );
    }

    draw(
        &mut canvas,
        "MasloMusicBot · genre dynamics",
        MARGIN as f32,
        HEIGHT as f32 - 12.0,
        12.0,
        MUTED,
        &font,
    );
    canvas
        .save(out)
        .map_err(|error| format!("cannot save dynamics chart: {error}"))
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
    fn renders_genre_bar_chart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("genres.png");
        let items: Vec<BarItem> = [
            ("Alternative Rock", 12),
            ("Hard Rock", 9),
            ("Metal", 8),
            ("Electronic", 5),
            ("Pop", 3),
        ]
        .into_iter()
        .map(|(label, value)| BarItem {
            label: label.to_owned(),
            value,
        })
        .collect();
        render_bars("Your genres", "25 tracks · 5 genres", &items, &path).expect("chart");
        let bytes = std::fs::read(&path).expect("png");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
        let decoded = image::load_from_memory(&bytes).expect("decode");
        assert!(decoded.width() >= 1_000 && decoded.height() > 600);
    }

    #[test]
    fn renders_genre_dynamics_chart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("dynamics.png");
        let labels: Vec<String> = (0..12).map(|i| format!("d{i:02}")).collect();
        let series = vec![
            LineSeries {
                label: "Rock".to_owned(),
                color: SERIES_COLORS[0],
                values: vec![1, 3, 2, 5, 4, 7, 6, 8, 5, 9, 7, 10],
            },
            LineSeries {
                label: "Jazz".to_owned(),
                color: SERIES_COLORS[1],
                values: vec![0, 1, 0, 2, 1, 3, 2, 2, 4, 3, 5, 4],
            },
        ];
        render_lines(
            "Genre dynamics",
            "24 cells · 12 buckets",
            &labels,
            &series,
            &path,
        )
        .expect("chart");
        let bytes = std::fs::read(&path).expect("png");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
        let decoded = image::load_from_memory(&bytes).expect("decode");
        assert!(decoded.width() >= 1_000 && decoded.height() >= 600);
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
                cover: Some(encoded.clone().into()),
            })
            .collect();
        render("Top albums", &items, 3, &path).expect("render");
        assert!(std::fs::metadata(&path).expect("png").len() > 10_000);
    }
}
