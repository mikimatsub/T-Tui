//! Native terminal photos, encoded off the UI thread and cached by actual cell
//! size. Ratatui's image widget owns damage tracking and screen cleanup.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use image::{DynamicImage, imageops::FilterType};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    style::Color,
    widgets::Widget,
};
use ratatui_image::{
    Image, Resize,
    picker::{Picker, ProtocolType},
    protocol::Protocol,
};

use crate::images::RenderedImage;

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    url: String,
    width: u16,
    height: u16,
    background: [u8; 4],
    avatar: bool,
}

enum Entry {
    Encoding,
    Ready(Arc<Protocol>, Instant),
    Failed,
}

struct State {
    picker: Picker,
    generation: u64,
    cache: HashMap<Key, Entry>,
}

pub struct Graphics {
    state: Arc<Mutex<State>>,
    permits: Arc<tokio::sync::Semaphore>,
}

impl Default for Graphics {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                picker: Picker::halfblocks(),
                generation: 0,
                cache: HashMap::new(),
            })),
            permits: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }
}

impl Graphics {
    pub fn detect(&self) {
        self.set_picker(detect_picker());
    }

    fn set_picker(&self, picker: Picker) {
        let mut state = self.state.lock().unwrap();
        state.picker = picker;
        state.generation += 1;
        state.cache.clear();
    }

    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.generation += 1;
        state.cache.clear();
    }

    pub fn label(&self) -> &'static str {
        match self.state.lock().unwrap().picker.protocol_type() {
            ProtocolType::Sixel => "Sixel",
            ProtocolType::Kitty => "Kitty",
            ProtocolType::Iterm2 => "iTerm2",
            ProtocolType::Halfblocks => "text fallback",
        }
    }

    /// A window/font resize can change pixels per cell. An ioctl does not
    /// consume stdin or race the input pump, unlike another capability query.
    pub fn update_cell_size(&self) {
        let Ok(size) = crossterm::terminal::window_size() else {
            return;
        };
        if size.columns == 0 || size.rows == 0 {
            return;
        }
        let width = size.width / size.columns;
        let height = size.height / size.rows;
        let mut state = self.state.lock().unwrap();
        let old = state.picker.font_size();
        if width == 0
            || height == 0
            || (width == old.width && height == old.height)
            || state.picker.protocol_type() == ProtocolType::Halfblocks
        {
            return;
        }
        // The protocol was already detected authoritatively at startup. Only
        // replace its cell geometry; do not infer a new protocol from the env.
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(ratatui_image::FontSize::new(width, height));
        picker.set_protocol_type(state.picker.protocol_type());
        state.picker = picker;
        state.generation += 1;
        state.cache.clear();
    }

    /// Returns false while the correctly sized native image is being prepared.
    /// No image decoding, resizing, or protocol encoding runs on the UI thread.
    pub fn draw(
        &self,
        url: &str,
        image: &RenderedImage,
        buf: &mut Buffer,
        area: Rect,
        background: Color,
        avatar: bool,
    ) -> bool {
        if area.is_empty() {
            return true;
        }
        let mut state = self.state.lock().unwrap();
        if state.picker.protocol_type() == ProtocolType::Halfblocks {
            image.draw_fit(buf, area);
            return true;
        }
        let background = match background {
            Color::Rgb(r, g, b) => [r, g, b, 255],
            _ => [16, 18, 24, 255],
        };
        let key = Key {
            url: url.to_owned(),
            width: area.width,
            height: area.height,
            background,
            avatar,
        };
        if let Some(entry) = state.cache.get_mut(&key) {
            return match entry {
                Entry::Ready(protocol, used) => {
                    *used = Instant::now();
                    let size = protocol.size();
                    let dest = Rect::new(
                        area.x + area.width.saturating_sub(size.width) / 2,
                        area.y + area.height.saturating_sub(size.height) / 2,
                        size.width,
                        size.height,
                    );
                    Image::new(protocol).render(dest, buf);
                    true
                }
                Entry::Encoding => false,
                Entry::Failed => {
                    image.draw_fit(buf, area);
                    true
                }
            };
        }
        // Keep encoded data bounded; skip excess requests during rapid resizing.
        if state.cache.len() >= 48 {
            let oldest = state
                .cache
                .iter()
                .filter_map(|(key, entry)| match entry {
                    Entry::Ready(_, used) => Some((key.clone(), *used)),
                    Entry::Failed => Some((key.clone(), Instant::now())),
                    Entry::Encoding => None,
                })
                .min_by_key(|(_, used)| *used)
                .map(|(key, _)| key);
            if let Some(oldest) = oldest {
                state.cache.remove(&oldest);
            } else {
                return false;
            }
        }
        let picker = state.picker.clone();
        let generation = state.generation;
        state.cache.insert(key.clone(), Entry::Encoding);
        drop(state);
        let source = image.source.clone();
        let state = self.state.clone();
        let permits = self.permits.clone();
        tokio::spawn(async move {
            let Ok(_permit) = permits.acquire_owned().await else {
                return;
            };
            if state.lock().unwrap().generation != generation {
                return;
            }
            let encode_key = key.clone();
            let result =
                tokio::task::spawn_blocking(move || encode(picker, &source, &encode_key)).await;
            let mut state = state.lock().unwrap();
            if state.generation == generation {
                let entry = match result {
                    Ok(Ok(protocol)) => Entry::Ready(Arc::new(protocol), Instant::now()),
                    _ => Entry::Failed,
                };
                state.cache.insert(key, entry);
            }
        });
        false
    }
}

/// Use a bounded, synchronous query. The library's convenience query leaves a
/// blocked stdin thread after a timeout; that thread can steal the user's first
/// keystroke in terminals which don't answer graphics queries.
#[cfg(unix)]
fn detect_picker() -> Picker {
    use ratatui_image::picker::cap_parser::{Parser, QueryStdioOptions, Response};
    use rustix::event::{PollFd, PollFlags, poll};
    use std::io::Write;
    use std::time::Duration;

    let fallback = Picker::halfblocks();
    let mut options = QueryStdioOptions::default();
    let wezterm = std::env::var_os("WEZTERM_EXECUTABLE").is_some();
    let konsole = std::env::var_os("KONSOLE_VERSION").is_some();
    if wezterm || konsole {
        options.blacklist_protocols = vec![ProtocolType::Kitty, ProtocolType::Sixel];
    }
    let query = Parser::query(fallback.tmux_detected(), options);
    let mut out = std::io::stdout();
    if out
        .write_all(query.as_bytes())
        .and_then(|_| out.flush())
        .is_err()
    {
        return fallback;
    }
    let stdin = std::io::stdin();
    let mut parser = Parser::new();
    let mut font = crossterm::terminal::window_size().ok().and_then(|s| {
        if s.columns == 0 || s.rows == 0 {
            return None;
        }
        let (w, h) = (s.width / s.columns, s.height / s.rows);
        (w > 0 && h > 0).then_some((w, h))
    });
    let mut protocol = None;
    let deadline = Instant::now() + Duration::from_millis(700);
    'query: while Instant::now() < deadline {
        let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .as_millis() as i32;
        if !matches!(poll(&mut fds, remaining.max(1)), Ok(n) if n > 0) {
            break;
        }
        let mut byte = [0u8; 1];
        if !matches!(rustix::io::read(&stdin, &mut byte), Ok(1)) {
            break;
        }
        for response in parser.push(char::from(byte[0])) {
            match response {
                Response::Status => break 'query,
                Response::CellSize(Some(size)) => font = Some(size),
                Response::Kitty => protocol = Some(ProtocolType::Kitty),
                Response::Sixel if protocol.is_none() => protocol = Some(ProtocolType::Sixel),
                _ => {}
            }
        }
    }
    if let Some((width, height)) = font.filter(|(w, h)| *w > 0 && *h > 0) {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(ratatui_image::FontSize::new(width, height));
        if let Some(protocol) = protocol {
            picker.set_protocol_type(protocol);
        }
        picker
    } else {
        fallback
    }
}

#[cfg(not(unix))]
fn detect_picker() -> Picker {
    Picker::halfblocks()
}

fn encode(
    mut picker: Picker,
    source: &DynamicImage,
    key: &Key,
) -> Result<Protocol, ratatui_image::errors::Errors> {
    let font = picker.font_size();
    let size = Size::new(
        key.width.min((2048 / font.width).max(1)),
        key.height.min((2048 / font.height).max(1)),
    );
    picker.set_background_color(Some(key.background));
    let source = if key.avatar {
        // Fill small avatar tiles; retain the entire image in large photo views.
        source.resize_to_fill(
            u32::from(size.width) * u32::from(font.width),
            u32::from(size.height) * u32::from(font.height),
            FilterType::Lanczos3,
        )
    } else {
        source.clone()
    };
    picker.new_protocol(source, size, Resize::Scale(Some(FilterType::Lanczos3)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::CellDiffOption;

    fn sixel_picker() -> Picker {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(ratatui_image::FontSize::new(10, 20));
        picker.set_protocol_type(ProtocolType::Sixel);
        picker
    }

    #[test]
    fn sixel_retains_pixel_detail_and_cleans_up_on_navigation() {
        let source = DynamicImage::new_rgb8(600, 800);
        let key = Key {
            url: "test".into(),
            width: 40,
            height: 20,
            background: [16, 18, 24, 255],
            avatar: false,
        };
        let protocol = encode(sixel_picker(), &source, &key).unwrap();
        assert_eq!(protocol.size(), Size::new(30, 20));
        let Protocol::Sixel(sixel) = &protocol else {
            panic!("expected native Sixel")
        };
        assert!(
            sixel.data.contains("\"1;1;300;400"),
            "must encode 300×400 pixels, not 30×40 colored blocks"
        );
        let mut previous = Buffer::empty(Rect::new(0, 0, 50, 24));
        Image::new(&protocol).render(Rect::new(4, 2, 30, 20), &mut previous);
        assert_eq!(previous[(5, 3)].diff_option, CellDiffOption::Skip);
        let next = Buffer::empty(previous.area);
        let diff = previous.diff(&next);
        assert!(
            diff.iter().any(|(x, y, _)| (*x, *y) == (5, 3)),
            "old graphics cells must be erased when navigating or opening a modal"
        );
    }

    #[tokio::test]
    async fn encoding_is_async_cached_and_resizes_without_stale_images() {
        let graphics = Graphics::default();
        graphics.set_picker(sixel_picker());
        let mut bytes = std::io::Cursor::new(Vec::new());
        DynamicImage::new_rgb8(120, 160)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let image = crate::images::render(bytes.get_ref(), 30, 20).unwrap();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 40));
        let area = Rect::new(2, 2, 20, 10);
        assert!(!graphics.draw("one", &image, &mut buf, area, Color::Black, false));
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while !graphics.draw("one", &image, &mut buf, area, Color::Black, false) {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(graphics.state.lock().unwrap().cache.len(), 1);
        assert!(!graphics.draw(
            "one",
            &image,
            &mut buf,
            Rect::new(2, 2, 10, 5),
            Color::Black,
            false
        ));
        graphics.clear();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(graphics.state.lock().unwrap().cache.is_empty());
    }
}
