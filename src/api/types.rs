use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};

/// A photo on a profile. The canonical full-size URL is in `url`; `processed_files`
/// and `assets` carry CDN renditions at various sizes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Photo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default, alias = "processed_files")]
    pub processed_files: Vec<ProcessedFile>,
    #[serde(default)]
    pub assets: Vec<Asset>,
    #[serde(default, alias = "media_type")]
    pub media_type: Option<String>,
    #[serde(default)]
    pub extension: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
}

impl Photo {
    /// Best URL to download for display: prefer a processed/asset rendition that is
    /// close to the display size, fall back to the full-size `url`.
    pub fn display_url(&self) -> Option<String> {
        if self.is_video() {
            return None;
        }
        // Prefer an asset/processed file around ~800px wide if available.
        let mut best: Option<(&ProcessedFile, f64)> = None;
        for pf in &self.processed_files {
            let score = (pf.width as f64 - 800.0).abs();
            if best.map(|(_, s)| score < s).unwrap_or(true) {
                best = Some((pf, score));
            }
        }
        if let Some((pf, _)) = best
            && !pf.url.is_empty()
        {
            return Some(pf.url.clone());
        }
        for a in &self.assets {
            if !a.url.is_empty() {
                // Prefer jpeg-ish assets; asset_type is like "jpg"/"png".
                return Some(a.url.clone());
            }
        }
        self.url.clone().filter(|u| !u.is_empty())
    }

    pub fn is_video(&self) -> bool {
        self.media_type.as_deref() == Some("video")
            || self.processed_files.iter().all(|p| p.url.contains("video"))
                && !self.processed_files.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProcessedFile {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub asset_type: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Person {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub bio: String,
    #[serde(default)]
    pub birth_date: Option<String>,
    #[serde(default)]
    pub gender: i32,
    #[serde(default)]
    pub photos: Vec<Photo>,
    #[serde(default)]
    pub ping_time: Option<String>,
    #[serde(default)]
    pub badges: Vec<Badge>,
}

impl Person {
    pub fn age(&self) -> Option<u32> {
        age_from_birth_date(self.birth_date.as_deref())
    }

    pub fn first_photo(&self) -> Option<&Photo> {
        self.photos.iter().find(|p| p.display_url().is_some())
    }
}

/// Extract an age in whole years from a birth date string ("1998-05-01", "May 1, 1998", ...).
pub fn age_from_birth_date(bd: Option<&str>) -> Option<u32> {
    let bd = bd?;
    let date = DateTime::parse_from_rfc3339(bd)
        .ok()
        .map(|d| d.date_naive())
        .or_else(|| chrono::NaiveDate::parse_from_str(bd, "%Y-%m-%d").ok())
        .or_else(|| chrono::NaiveDate::parse_from_str(bd, "%B %e, %Y").ok())?;
    let now = Utc::now().date_naive();
    let years =
        now.year() - date.year() - i32::from((now.month(), now.day()) < (date.month(), date.day()));
    (years >= 0).then_some(years as u32)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Badge {
    #[serde(default)]
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReadReceipt {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(default, rename = "_id", alias = "id")]
    pub id: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    /// Unix milliseconds.
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub match_id: Option<String>,
    #[serde(default)]
    pub sent_date: Option<String>,
}

impl Message {
    pub fn time(&self) -> Option<DateTime<Utc>> {
        if self.timestamp > 0 {
            return DateTime::from_timestamp_millis(self.timestamp);
        }
        parse_datetime(self.sent_date.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Match {
    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub dead: bool,
    #[serde(default)]
    pub created_date: Option<String>,
    #[serde(default)]
    pub last_activity_date: Option<String>,
    #[serde(default)]
    pub message_count: i32,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default)]
    pub person: Option<Person>,
    #[serde(default)]
    pub readreceipt: Option<ReadReceipt>,
    #[serde(default)]
    pub subscription_tier: Option<String>,
    /// Present in `/updates` payloads (new messages for this match).
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub is_new_match: bool,
}

impl Match {
    pub fn last_activity(&self) -> Option<DateTime<Utc>> {
        parse_datetime(self.last_activity_date.as_deref())
    }

    /// The participant that is not `own_id` (i.e. the other person in the match).
    pub fn other_id(&self, own_id: &str) -> Option<String> {
        if let Some(p) = &self.person
            && !p.id.is_empty()
            && p.id != own_id
        {
            return Some(p.id.clone());
        }
        self.participants.iter().find(|p| *p != own_id).cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SexualOrientation {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Prompt {
    #[serde(default)]
    pub question_text: Option<String>,
    #[serde(default)]
    pub answer_text: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserPrompts {
    #[serde(default)]
    pub section_name: Option<String>,
    #[serde(default)]
    pub prompts: Vec<Prompt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Descriptor {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelationshipIntent {
    #[serde(default)]
    pub emoji: Option<String>,
    #[serde(default)]
    pub title_text: Option<String>,
    #[serde(default)]
    pub body_text: Option<String>,
}

/// A full user profile (own or another user's).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserProfile {
    #[serde(default)]
    pub age_filter_min: Option<i32>,
    #[serde(default)]
    pub age_filter_max: Option<i32>,
    #[serde(default)]
    pub distance_filter: Option<i32>,
    #[serde(default)]
    pub gender_filter: Option<i32>,
    #[serde(default)]
    pub discoverable: Option<bool>,

    #[serde(default, rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub bio: String,
    #[serde(default)]
    pub birth_date: Option<String>,
    #[serde(default)]
    pub birth_date_info: Option<String>,
    #[serde(default)]
    pub gender: i32,
    #[serde(default)]
    pub show_gender_on_profile: bool,
    #[serde(default)]
    pub distance_mi: Option<i32>,
    #[serde(default)]
    pub photos: Vec<Photo>,
    #[serde(default)]
    pub jobs: Vec<serde_json::Value>,
    #[serde(default)]
    pub schools: Vec<serde_json::Value>,
    #[serde(default)]
    pub sexual_orientations: Vec<SexualOrientation>,
    #[serde(default)]
    pub selected_descriptors: Vec<Descriptor>,
    #[serde(default)]
    pub relationship_intent: Option<RelationshipIntent>,
    #[serde(default)]
    pub user_prompts: Option<UserPrompts>,
    #[serde(default)]
    pub common_like_count: i32,
    #[serde(default)]
    pub common_interests: Vec<serde_json::Value>,
}

impl UserProfile {
    pub fn age(&self) -> Option<u32> {
        age_from_birth_date(
            self.birth_date
                .as_deref()
                .or(self.birth_date_info.as_deref()),
        )
    }

    /// A short one-line summary used in lists.
    pub fn summary(&self) -> String {
        let mut parts = vec![self.name.clone()];
        if let Some(a) = self.age() {
            parts.push(a.to_string());
        }
        if let Some(d) = self.distance_mi {
            parts.push(format!("{d} mi"));
        }
        parts.join(" · ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PollInterval {
    #[serde(default)]
    pub standard: i64,
    #[serde(default)]
    pub persistent: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateResponse {
    #[serde(default)]
    pub matches: Vec<Match>,
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub inbox: Vec<serde_json::Value>,
    #[serde(default)]
    pub liked_messages: Vec<serde_json::Value>,
    #[serde(default)]
    pub harassing_messages: Vec<serde_json::Value>,
    #[serde(default)]
    pub last_activity_date: Option<String>,
    #[serde(default)]
    pub poll_interval: Option<PollInterval>,
}

/// Result of a token refresh.
#[derive(Debug, Clone, Default)]
pub struct RefreshResult {
    pub auth_token: String,
    pub refresh_token: Option<String>,
    pub user_id: Option<String>,
    pub ttl_ms: Option<i64>,
}

/// Envelope for most REST endpoints: `{ "meta": {...}, "data": {...} }`.
#[derive(Debug, Deserialize)]
pub struct Envelope<T> {
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
    #[serde(default)]
    pub data: Option<T>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MatchesData {
    #[serde(default)]
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub matches: Vec<Match>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MessagesData {
    #[serde(default)]
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub messages: Vec<Message>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UserEnvelope {
    #[serde(default)]
    pub user: Option<UserProfile>,
}

/// Response of `GET /user/{id}`: `{ "status": 200, "results": { ...profile } }`.
#[derive(Debug, Deserialize)]
pub struct UserProfileResponse {
    #[serde(default)]
    pub status: i32,
    #[serde(default)]
    pub results: Option<UserProfile>,
}

/// Parse an ISO-8601 / RFC3339 timestamp string into UTC.
pub fn parse_datetime(s: Option<&str>) -> Option<DateTime<Utc>> {
    let s = s?;
    if s.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f%:z")
                .ok()
                .map(|ndt| ndt.and_utc())
        })
}

/// Format a UTC datetime the way the API expects in `last_activity_date`.
pub fn format_activity(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Recommendation {
    #[serde(default)]
    pub user: UserProfile,
    #[serde(default)]
    pub s_number: Option<serde_json::Value>,
    #[serde(default)]
    pub distance_mi: Option<i32>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RecommendationsData {
    #[serde(default)]
    pub results: Vec<Recommendation>,
}

#[derive(Debug, Clone, Default)]
pub struct SwipeResult {
    pub matched: bool,
    pub new_match: Option<Match>,
    pub likes_remaining: Option<u64>,
}

/// Partial account update. None means leave the server value unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_filter_min: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_filter_max: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_filter: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender_filter: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discoverable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_gender_on_profile: Option<bool>,
}
impl ProfileUpdate {
    pub fn validate(&self) -> Result<(), String> {
        if self.bio.as_ref().is_some_and(|s| s.chars().count() > 500) {
            return Err("Bio must be at most 500 characters.".into());
        }
        if [self.age_filter_min, self.age_filter_max]
            .into_iter()
            .flatten()
            .any(|n| !(18..=100).contains(&n))
        {
            return Err("Ages must be between 18 and 100.".into());
        }
        if let (Some(min), Some(max)) = (self.age_filter_min, self.age_filter_max)
            && min > max
        {
            return Err("Minimum age cannot exceed maximum age.".into());
        }
        if self
            .distance_filter
            .is_some_and(|n| !(1..=100).contains(&n))
        {
            return Err("Distance must be between 1 and 100 miles.".into());
        }
        if self.gender_filter.is_some_and(|n| ![-1, 0, 1].contains(&n)) {
            return Err("Invalid gender preference.".into());
        }
        Ok(())
    }
    pub fn apply(&self, user: &mut UserProfile) {
        if let Some(bio) = &self.bio {
            user.bio = bio.clone();
        }
        if let Some(v) = self.age_filter_min {
            user.age_filter_min = Some(v);
        }
        if let Some(v) = self.age_filter_max {
            user.age_filter_max = Some(v);
        }
        if let Some(v) = self.distance_filter {
            user.distance_filter = Some(v);
        }
        if let Some(v) = self.gender_filter {
            user.gender_filter = Some(v);
        }
        if let Some(v) = self.discoverable {
            user.discoverable = Some(v);
        }
        if let Some(v) = self.show_gender_on_profile {
            user.show_gender_on_profile = v;
        }
    }
}
