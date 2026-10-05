//! Bounded image decoding. Retain detailed pixels for native terminal graphics,
//! with a small half-block preview for terminals without graphics support.

use image::ImageDecoder;
use std::sync::Arc;

use ratatui::buffer::Buffer;
#[cfg(test)]
use ratatui::buffer::Cell;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// A raster image pre-mapped to terminal cells.
#[derive(Debug, Clone)]
pub struct RenderedImage {
    pub source: Arc<image::DynamicImage>,
    pub width: u16,
    pub height: u16,
    /// One (fg, bg) pair per cell, row-major. fg is the top pixel of the
    /// cell, bg the bottom pixel.
    pub cells: Vec<(Color, Color)>,
}

impl RenderedImage {
    pub fn source_bytes(&self) -> usize {
        self.source.as_bytes().len()
    }
    /// Draw into `buf`, centered inside `area`. Pixels outside the area are
    /// clipped. Uses the half-block glyph so both colors show.
    #[cfg(test)]
    pub fn draw(&self, buf: &mut Buffer, area: Rect) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let ox = area
            .x
            .saturating_add((area.width.saturating_sub(self.width)) / 2);
        let oy = area
            .y
            .saturating_add((area.height.saturating_sub(self.height)) / 2);
        for (y, row) in self.cells.chunks_exact(self.width as usize).enumerate() {
            let ty = oy + y as u16;
            if ty >= area.y + area.height || ty >= buf.area().y + buf.area().height {
                continue;
            }
            for (x, &(fg, bg)) in row.iter().enumerate() {
                let tx = ox + x as u16;
                if tx >= area.x + area.width || tx >= buf.area().x + buf.area().width {
                    continue;
                }
                let mut cell = Cell::default();
                cell.set_char('▀');
                cell.set_fg(fg);
                cell.set_bg(bg);
                buf[(tx, ty)] = cell;
            }
        }
    }

    /// Fit the already-decoded image into smaller terminals without cropping.
    pub fn draw_fit(&self, buf: &mut Buffer, area: Rect) {
        if self.width == 0 || self.height == 0 || area.is_empty() {
            return;
        }
        let scale = (area.width as f64 / self.width as f64)
            .min(area.height as f64 / self.height as f64)
            .min(1.0);
        let w = ((self.width as f64 * scale) as u16).max(1);
        let h = ((self.height as f64 * scale) as u16).max(1);
        let x0 = area.x + (area.width - w) / 2;
        let y0 = area.y + (area.height - h) / 2;
        for y in 0..h {
            for x in 0..w {
                let sx = x as usize * self.width as usize / w as usize;
                let sy = y as usize * self.height as usize / h as usize;
                let (fg, bg) = self.cells[sy * self.width as usize + sx];
                if let Some(cell) = buf.cell_mut((x0 + x, y0 + y)) {
                    cell.set_char('▀').set_fg(fg).set_bg(bg);
                }
            }
        }
    }

    /// Solid-color image (used for loading placeholders).
    #[cfg(test)]
    pub fn solid(width: u16, height: u16, color: Color) -> Self {
        Self {
            source: Arc::new(image::DynamicImage::new_rgb8(
                width as u32,
                height as u32 * 2,
            )),
            width,
            height,
            cells: vec![(color, color); (width as usize) * (height as usize)],
        }
    }
}

/// Decode and downscale image bytes to a `RenderedImage` of at most
/// `width x height` terminal cells. Aspect ratio is preserved; the result is
/// centered by `draw`.
pub fn render(bytes: &[u8], width: u16, height: u16) -> Option<RenderedImage> {
    if width == 0 || height == 0 {
        return None;
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = image::DynamicImage::from_decoder(decoder).ok()?;
    img.apply_orientation(orientation);
    // More than enough detail for a full-screen terminal photo, while bounding
    // memory independently of the resolution supplied by the CDN.
    if img.width() > 2048 || img.height() > 2048 {
        img = img.resize(2048, 2048, image::imageops::FilterType::Lanczos3);
    }

    // Target pixel size: full cell width, double the cell height (half-blocks).
    let target_w = width as u32;
    let target_h = (height as u32).saturating_mul(2);
    if target_h == 0 {
        return None;
    }

    // Preserve the source proportions: terminal cells are approximately 1:2.
    let scaled = img
        .resize(
            target_w.max(1),
            target_h.max(2),
            image::imageops::FilterType::Lanczos3,
        )
        .to_rgb8();
    if scaled.height() == 0 || scaled.width() == 0 {
        return None;
    }

    // Snap to even height so every cell has a top and bottom pixel.
    let h = scaled.height().div_ceil(2) * 2;
    let w = scaled.width();

    let mut cells = Vec::with_capacity((w as usize) * ((h / 2) as usize));
    for y in 0..(h / 2) {
        for x in 0..w {
            let top = scaled.get_pixel(x, y * 2);
            let bottom = scaled.get_pixel(x, (y * 2 + 1).min(scaled.height() - 1));
            cells.push((
                Color::Rgb(top[0], top[1], top[2]),
                Color::Rgb(bottom[0], bottom[1], bottom[2]),
            ));
        }
    }

    Some(RenderedImage {
        source: Arc::new(img),
        width: w as u16,
        height: (h / 2) as u16,
        cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;

    fn make_png(w: u32, h: u32, color: [u8; 3]) -> Vec<u8> {
        let img = RgbImage::from_pixel(w, h, image::Rgb(color));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn render_scales_down() {
        let bytes = make_png(800, 600, [200, 100, 50]);
        let img = render(&bytes, 40, 20).unwrap();
        assert!(img.width <= 40);
        assert!(img.height <= 20);
        // Aspect preserved: width should be the binding constraint.
        assert_eq!(img.width, 40);
        assert_eq!(img.height, 15);
        let (fg, bg) = img.cells[0];
        assert_eq!(
            (fg, bg),
            (Color::Rgb(200, 100, 50), Color::Rgb(200, 100, 50))
        );
    }

    #[test]
    fn render_rejects_garbage() {
        assert!(render(b"not an image", 10, 5).is_none());
    }

    #[test]
    fn draw_centers_and_clips() {
        let img = RenderedImage::solid(4, 2, Color::Red);
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
        img.draw(&mut buf, Rect::new(0, 0, 10, 5));
        // Centered: x offset (10-4)/2 = 3, y offset (5-2)/2 = 1.
        assert_eq!(buf[(3, 1)].symbol(), "▀");
        assert_eq!(buf[(6, 2)].fg, Color::Red);
        // Outside the image stays default.
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }
}
