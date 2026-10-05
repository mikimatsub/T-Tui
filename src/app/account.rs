//! Account edits remain local until Save. Omitted server values are never invented.
use crate::api::types::{ProfileUpdate, UserProfile};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;

pub const LABELS: [&str; 7] = [
    "Bio",
    "Minimum age",
    "Maximum age",
    "Distance (miles)",
    "Show me",
    "Show me in Discovery",
    "Show gender on profile",
];
#[derive(Debug, Default)]
pub struct AccountEditor {
    pub fields: Vec<String>,
    original: Vec<String>,
    pub focus: usize,
    pub cursor: usize,
    pub loading: bool,
    pub saving: bool,
    pub error: Option<String>,
}
impl AccountEditor {
    pub fn load(&mut self, user: &UserProfile) {
        let number = |v: Option<i32>| v.map(|n| n.to_string()).unwrap_or_default();
        let boolean = |v: Option<bool>| {
            v.map(|b| if b { "On" } else { "Off" }.into())
                .unwrap_or_default()
        };
        self.fields = vec![
            user.bio.clone(),
            number(user.age_filter_min),
            number(user.age_filter_max),
            number(user.distance_filter),
            user.gender_filter
                .map(|g| {
                    match g {
                        0 => "Men",
                        1 => "Women",
                        -1 => "Everyone",
                        _ => "Unknown",
                    }
                    .into()
                })
                .unwrap_or_default(),
            boolean(user.discoverable),
            boolean(Some(user.show_gender_on_profile)),
        ];
        self.original = self.fields.clone();
        self.cursor = self.fields[self.focus].len();
        self.error = None;
        self.loading = false;
    }
    pub fn dirty(&self) -> bool {
        self.fields != self.original
    }
    pub fn select(&mut self, focus: usize) {
        if self.saving || self.loading || self.fields.is_empty() {
            return;
        }
        self.focus = focus.min(LABELS.len() - 1);
        self.cursor = self.fields[self.focus].len();
    }
    pub fn insert(&mut self, s: &str) {
        if self.saving || self.loading || self.fields.is_empty() || self.focus > 3 {
            return;
        }
        let value = &mut self.fields[self.focus];
        let limit: usize = if self.focus == 0 { 500 } else { 3 };
        let clean: String = s
            .chars()
            .filter(|c| {
                if self.focus == 0 {
                    !c.is_control() || *c == '\n'
                } else {
                    c.is_ascii_digit()
                }
            })
            .take(limit.saturating_sub(value.chars().count()))
            .collect();
        value.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }
    pub fn change_choice(&mut self) {
        if self.saving || self.loading || self.fields.is_empty() {
            return;
        }
        let values: &[&str] = match self.focus {
            4 => &["Everyone", "Men", "Women"],
            5 | 6 => &["On", "Off"],
            _ => return,
        };
        let pos = values.iter().position(|v| *v == self.fields[self.focus]);
        self.fields[self.focus] = values[pos.map(|n| (n + 1) % values.len()).unwrap_or(0)].into();
    }
    pub fn key(&mut self, key: KeyEvent) {
        if self.saving || self.loading || self.fields.is_empty() {
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Tab | KeyCode::Down => self.select((self.focus + 1) % LABELS.len()),
            KeyCode::BackTab | KeyCode::Up => {
                self.select((self.focus + LABELS.len() - 1) % LABELS.len())
            }
            KeyCode::Char('u') if ctrl && self.focus <= 3 => {
                self.fields[self.focus].clear();
                self.cursor = 0;
            }
            KeyCode::Char(' ') | KeyCode::Enter if self.focus > 3 => self.change_choice(),
            KeyCode::Enter if self.focus == 0 => self.insert("\n"),
            KeyCode::Char(c) if !ctrl => self.insert(&c.to_string()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.fields[self.focus].len(),
            KeyCode::Left => {
                self.cursor = super::previous_boundary(&self.fields[self.focus], self.cursor)
            }
            KeyCode::Right if self.focus > 3 => self.change_choice(),
            KeyCode::Right => {
                self.cursor = super::next_boundary(&self.fields[self.focus], self.cursor)
            }
            KeyCode::Backspace if self.cursor > 0 && self.focus <= 3 => {
                let prev = super::previous_boundary(&self.fields[self.focus], self.cursor);
                self.fields[self.focus].drain(prev..self.cursor);
                self.cursor = prev;
            }
            KeyCode::Delete if self.focus <= 3 => {
                let next = super::next_boundary(&self.fields[self.focus], self.cursor);
                self.fields[self.focus].drain(self.cursor..next);
            }
            _ => {}
        }
    }
    pub fn update(&self) -> Result<ProfileUpdate, String> {
        if self.fields.len() != LABELS.len() {
            return Err("Wait for your account to load.".into());
        }
        let changed = |i| self.fields[i] != self.original[i];
        let parse = |i: usize, min: i32, max: i32| -> Result<Option<i32>, String> {
            if !changed(i) {
                return Ok(None);
            }
            self.fields[i]
                .parse::<i32>()
                .ok()
                .filter(|n| (min..=max).contains(n))
                .map(Some)
                .ok_or_else(|| format!("{} must be between {min} and {max}.", LABELS[i]))
        };
        let update = ProfileUpdate {
            bio: changed(0).then(|| self.fields[0].clone()),
            age_filter_min: parse(1, 18, 100)?,
            age_filter_max: parse(2, 18, 100)?,
            distance_filter: parse(3, 1, 100)?,
            gender_filter: changed(4).then(|| match self.fields[4].as_str() {
                "Men" => 0,
                "Women" => 1,
                _ => -1,
            }),
            discoverable: changed(5).then(|| self.fields[5] == "On"),
            show_gender_on_profile: changed(6).then(|| self.fields[6] == "On"),
        };
        if let (Ok(min), Ok(max)) = (self.fields[1].parse::<i32>(), self.fields[2].parse::<i32>())
            && (changed(1) || changed(2))
            && min > max
        {
            return Err("Minimum age cannot exceed maximum age.".into());
        }
        update.validate()?;
        Ok(update)
    }
    pub fn discard(&mut self) {
        if self.saving || self.loading {
            return;
        }
        self.fields = self.original.clone();
        self.select(self.focus);
        self.error = None;
    }
}

/// Byte offset at a cell in the same soft-wrapped layout used by the composer.
pub fn cursor_at(input: &str, width: usize, target_x: usize, target_y: usize) -> usize {
    let mut x = 0;
    let mut y = 0;
    for (idx, g) in input.grapheme_indices(true) {
        let w = unicode_width::UnicodeWidthStr::width(g);
        if g != "\n" && x + w >= width && x > 0 {
            x = 0;
            y += 1;
        }
        if y > target_y || (y == target_y && (x >= target_x || (g != "\n" && target_x < x + w))) {
            return idx;
        }
        if g == "\n" {
            if y == target_y {
                return idx;
            }
            x = 0;
            y += 1;
        } else {
            x += w;
        }
    }
    input.len()
}
