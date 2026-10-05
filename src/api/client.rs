use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;

use super::proto;
use super::types::{
    Envelope, Match, MatchesData, Message, MessagesData, ProfileUpdate, Recommendation,
    RecommendationsData, RefreshResult, SwipeResult, UpdateResponse, UserEnvelope, UserProfile,
    UserProfileResponse, format_activity,
};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("network error: {0}")]
    Network(String),
    #[error("authentication failed or expired (HTTP 401). Please sign in again.")]
    Auth,
    #[error("rate limited by the server (HTTP 429). Slowing down.")]
    RateLimited,
    #[error("server error (HTTP {code}): {body}")]
    Status { code: u16, body: String },
    #[error("failed to decode response: {0}")]
    Decode(String),
    #[error("{0}")]
    Other(String),
}

impl ApiError {
    pub fn is_auth(&self) -> bool {
        matches!(self, ApiError::Auth)
    }
}

const BASE: &str = "https://api.gotinder.com";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
     AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.3.1 Safari/605.1.15";

#[derive(Debug, Default)]
struct AuthState {
    auth_token: Option<String>,
    device_id: Option<String>,
    refresh_token: Option<String>,
    user_id: Option<String>,
}

#[derive(Clone)]
pub struct RealApi {
    http: reqwest::Client,
    base: String,
    auth: Arc<RwLock<AuthState>>,
}

#[async_trait]
pub trait TinderApi: Send + Sync {
    async fn update_profile(&self, update: &ProfileUpdate) -> Result<(), ApiError>;
    async fn super_like(
        &self,
        rec: &Recommendation,
    ) -> Result<(SwipeResult, Option<u64>), ApiError>;
    async fn boost(&self) -> Result<Option<u64>, ApiError>;
    async fn unmatch(&self, match_id: &str) -> Result<(), ApiError>;
    async fn get_recommendations(&self) -> Result<Vec<Recommendation>, ApiError>;
    async fn swipe(&self, rec: &Recommendation, like: bool) -> Result<SwipeResult, ApiError>;
    async fn get_own_user(&self) -> Result<UserProfile, ApiError>;
    async fn get_matches(
        &self,
        count: u32,
        only_with_messages: bool,
    ) -> Result<Vec<Match>, ApiError>;
    /// Page with the opaque cursor supplied by the server.
    async fn get_messages(
        &self,
        match_id: &str,
        count: u32,
        page_token: Option<String>,
    ) -> Result<MessagesData, ApiError>;
    async fn send_message(
        &self,
        own_id: &str,
        other_id: &str,
        match_id: &str,
        text: &str,
    ) -> Result<Message, ApiError>;
    async fn get_updates(&self, since: DateTime<Utc>) -> Result<UpdateResponse, ApiError>;
    async fn get_user(&self, user_id: &str) -> Result<UserProfile, ApiError>;
    async fn refresh_token(&self, refresh_token: &str) -> Result<RefreshResult, ApiError>;
    async fn download_photo(&self, url: &str) -> Result<Vec<u8>, ApiError>;
    /// Apply freshly obtained credentials to the in-memory auth state.
    /// Default is a no-op (e.g. for the mock backend).
    fn apply_auth(&self, _token: String, _device_id: String, _refresh_token: Option<String>) {}
}

impl RealApi {
    pub fn new(config: &crate::config::Config) -> Result<Self, ApiError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(USER_AGENT)
            .build()
            .map_err(|e| ApiError::Other(format!("building http client: {e}")))?;

        let auth = Arc::new(RwLock::new(AuthState {
            auth_token: config.auth_token.clone(),
            device_id: config.device_id.clone(),
            refresh_token: config.refresh_token.clone(),
            user_id: config.user_id.clone(),
        }));

        Ok(Self {
            http,
            auth,
            base: BASE.into(),
        })
    }

    /// Update the in-memory auth state (called after a successful login/refresh).
    pub fn set_auth(&self, token: String, device_id: String, refresh_token: Option<String>) {
        let mut st = self.auth.write().unwrap();
        st.auth_token = Some(token);
        st.device_id = Some(device_id);
        st.refresh_token = refresh_token;
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut h = reqwest::header::HeaderMap::new();
        let st = self.auth.read().unwrap();

        let mut set = |k: &str, v: &str| {
            if let (Ok(kv), Ok(kk)) = (
                v.parse::<reqwest::header::HeaderValue>(),
                k.parse::<reqwest::header::HeaderName>(),
            ) {
                h.insert(kk, kv);
            }
        };

        set("Accept", "application/json");
        set("Content-Type", "application/json");
        set("Accept-Language", "en,en-US");
        set("app-version", "1051800");
        set("platform", "web");
        set("support-short-video", "1");
        set("tinder-version", "5.18.0");
        set("x-supported-image-formats", "jpeg");
        set("User-Agent", USER_AGENT);
        if let Some(tok) = &st.auth_token {
            set("X-Auth-Token", tok);
        }
        if let Some(did) = &st.device_id {
            set("persistent-device-id", did);
        }
        h
    }

    async fn execute(&self, req: reqwest::RequestBuilder) -> Result<reqwest::Response, ApiError> {
        let resp = req
            .send()
            .await
            .map_err(|e| ApiError::Network(e.without_url().to_string()))?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body = resp.text().await.unwrap_or_default();
        let body = truncate(&body, 400);
        match status.as_u16() {
            401 => Err(ApiError::Auth),
            429 => Err(ApiError::RateLimited),
            code => Err(ApiError::Status { code, body }),
        }
    }
}

#[async_trait]
impl TinderApi for RealApi {
    async fn super_like(
        &self,
        rec: &Recommendation,
    ) -> Result<(SwipeResult, Option<u64>), ApiError> {
        let mut body = serde_json::Map::new();
        if let Some(n) = &rec.s_number {
            body.insert("s_number".into(), n.clone());
        }
        if let Some(photo) = rec.user.photos.first() {
            body.insert("liked_content_id".into(), photo.id.clone().into());
            body.insert("liked_content_type".into(), "photo".into());
        }
        let response = self
            .execute(
                self.http
                    .post(format!("{}/like/{}/super", self.base, rec.user.id))
                    .headers(self.headers())
                    .query(&[("locale", "en")])
                    .json(&body),
            )
            .await?;
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        check_mutation(&value)?;
        if value.get("match").is_none() || value.get("super_likes").is_none() {
            return Err(ApiError::Decode(
                "Super Like was not confirmed. Check Tinder before retrying.".into(),
            ));
        }
        let remaining = value
            .pointer("/super_likes/remaining")
            .and_then(|n| n.as_u64());
        let m = value.get("match");
        Ok((
            SwipeResult {
                matched: m.is_some_and(|m| m.as_bool() == Some(true) || m.is_object()),
                new_match: m
                    .filter(|m| m.is_object())
                    .and_then(|m| serde_json::from_value(m.clone()).ok()),
                likes_remaining: None,
            },
            remaining,
        ))
    }
    async fn boost(&self) -> Result<Option<u64>, ApiError> {
        let response = self
            .execute(
                self.http
                    .post(format!("{}/boost", self.base))
                    .headers(self.headers())
                    .query(&[("locale", "en")])
                    .json(&serde_json::json!({})),
            )
            .await?;
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        check_mutation(&value)?;
        if value
            .get("boost_id")
            .and_then(|s| s.as_str())
            .is_none_or(str::is_empty)
        {
            return Err(ApiError::Decode(
                "Boost activation was not confirmed. Check Tinder before retrying.".into(),
            ));
        }
        Ok(value.get("remaining").and_then(|n| n.as_u64()))
    }

    async fn update_profile(&self, update: &ProfileUpdate) -> Result<(), ApiError> {
        update.validate().map_err(ApiError::Other)?;
        let response = self
            .execute(
                self.http
                    .post(format!("{}/v2/profile", self.base))
                    .headers(self.headers())
                    .query(&[("locale", "en")])
                    .json(&serde_json::json!({"user": update})),
            )
            .await?;
        mutation_ack(response).await
    }
    async fn unmatch(&self, match_id: &str) -> Result<(), ApiError> {
        let mut url = url::Url::parse(&self.base).map_err(|e| ApiError::Other(e.to_string()))?;
        url.path_segments_mut()
            .map_err(|_| ApiError::Other("Invalid API URL".into()))?
            .extend(["user", "matches", match_id]);
        let response = self
            .execute(self.http.delete(url).headers(self.headers()))
            .await?;
        mutation_ack(response).await
    }

    async fn get_recommendations(&self) -> Result<Vec<Recommendation>, ApiError> {
        let resp = self
            .execute(
                self.http
                    .get(format!("{}/v2/recs/core", self.base))
                    .headers(self.headers())
                    .query(&[("locale", "en"), ("duos", "0")]),
            )
            .await?;
        let env: Envelope<RecommendationsData> = resp
            .json()
            .await
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        let data = envelope_data(env)?;
        Ok(data
            .results
            .into_iter()
            .filter(|r| !r.user.id.is_empty())
            .map(|mut r| {
                if r.user.distance_mi.is_none() {
                    r.user.distance_mi = r.distance_mi;
                }
                r
            })
            .collect())
    }

    async fn swipe(&self, rec: &Recommendation, like: bool) -> Result<SwipeResult, ApiError> {
        let route = if like { "like" } else { "pass" };
        let url = format!("{}/{route}/{}", self.base, rec.user.id);
        let request = if like {
            let mut body = serde_json::Map::new();
            if let Some(n) = &rec.s_number {
                body.insert("s_number".into(), n.clone());
            }
            if let Some(photo) = rec.user.photos.first() {
                body.insert("liked_content_id".into(), photo.id.clone().into());
                body.insert("liked_content_type".into(), "photo".into());
            }
            self.http.post(url).json(&body)
        } else {
            let mut req = self.http.get(url);
            if let Some(n) = &rec.s_number {
                req = req.query(&[(
                    "s_number",
                    n.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| n.to_string()),
                )]);
            }
            req
        };
        let resp = self
            .execute(request.headers(self.headers()).query(&[("locale", "en")]))
            .await?;
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        let status = v
            .get("status")
            .and_then(|s| s.as_u64())
            .or_else(|| v.pointer("/meta/status").and_then(|s| s.as_u64()));
        if status.is_some_and(|s| s >= 400) || v.get("error").is_some_and(|e| !e.is_null()) {
            return Err(ApiError::Other(
                "Swipe was rejected. Refresh discovery before trying again.".into(),
            ));
        }
        let m = v.get("match");
        Ok(SwipeResult {
            matched: m.is_some_and(|m| m.as_bool() == Some(true) || m.is_object()),
            new_match: m
                .filter(|m| m.is_object())
                .and_then(|m| serde_json::from_value(m.clone()).ok()),
            likes_remaining: v.get("likes_remaining").and_then(|n| n.as_u64()),
        })
    }

    fn apply_auth(&self, token: String, device_id: String, refresh_token: Option<String>) {
        self.set_auth(token, device_id, refresh_token);
    }

    async fn get_own_user(&self) -> Result<UserProfile, ApiError> {
        let url = format!("{}/v2/profile?locale=en&include=user", self.base);
        let resp = self
            .execute(self.http.get(&url).headers(self.headers()))
            .await?;
        let env: Envelope<UserEnvelope> = resp
            .json()
            .await
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        let user = envelope_data(env)?
            .user
            .filter(|u| !u.id.is_empty())
            .ok_or_else(|| ApiError::Decode("no user in profile response".into()))?;
        // Remember our own id.
        if let Ok(mut st) = self.auth.write()
            && !user.id.is_empty()
        {
            st.user_id = Some(user.id.clone());
        }
        Ok(user)
    }

    async fn get_matches(
        &self,
        count: u32,
        only_with_messages: bool,
    ) -> Result<Vec<Match>, ApiError> {
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        let mut visited = std::collections::HashSet::new();
        loop {
            let mut req = self
                .http
                .get(format!("{}/v2/matches", self.base))
                .headers(self.headers())
                .query(&[
                    ("locale", "en"),
                    ("count", &count.clamp(1, 100).to_string()),
                    ("is_tinder_u", "false"),
                    ("message", "1"),
                ]);
            if let Some(t) = &token {
                req = req.query(&[("page_token", t)]);
            }
            let resp = self.execute(req).await?;
            let env: Envelope<MatchesData> = resp
                .json()
                .await
                .map_err(|e| ApiError::Decode(e.to_string()))?;
            let page = envelope_data(env)?;
            for m in page.matches {
                if !out.iter().any(|old: &Match| old.id == m.id) {
                    out.push(m);
                }
            }
            token = page.next_page_token.filter(|t| !t.is_empty());
            if out.len() >= count as usize
                || token.as_ref().is_none_or(|t| !visited.insert(t.clone()))
            {
                break;
            }
        }
        out.truncate(count as usize);
        if only_with_messages {
            out.retain(|m| m.message_count > 0);
        }
        Ok(out)
    }

    async fn get_messages(
        &self,
        match_id: &str,
        count: u32,
        page_token: Option<String>,
    ) -> Result<MessagesData, ApiError> {
        let url = format!("{}/v2/matches/{match_id}/messages", self.base);
        let mut req = self
            .http
            .get(url)
            .headers(self.headers())
            .query(&[("locale", "en"), ("count", &count.to_string())]);
        if let Some(token) = page_token {
            req = req.query(&[("page_token", token)]);
        }
        let resp = self.execute(req).await?;
        let env: Envelope<MessagesData> = resp
            .json()
            .await
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        let mut page = envelope_data(env)?;
        page.next_page_token = page.next_page_token.filter(|s| !s.is_empty());
        Ok(page)
    }

    async fn send_message(
        &self,
        own_id: &str,
        other_id: &str,
        match_id: &str,
        text: &str,
    ) -> Result<Message, ApiError> {
        let url = format!("{}/user/matches/{match_id}?locale=en", self.base);
        let body = serde_json::json!({
            "userId": own_id,
            "otherId": other_id,
            "matchId": match_id,
            "sessionId": "",
            "message": text,
        });
        let resp = self
            .execute(self.http.post(&url).headers(self.headers()).json(&body))
            .await?;
        let mut msg = resp
            .json::<Message>()
            .await
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        if msg.id.is_empty() {
            return Err(ApiError::Decode(
                "message was not acknowledged; refresh the chat before retrying".into(),
            ));
        }
        msg.from.get_or_insert_with(|| own_id.to_owned());
        msg.to.get_or_insert_with(|| other_id.to_owned());
        if msg.message.is_empty() {
            msg.message = text.into();
        }
        Ok(msg)
    }

    async fn get_updates(&self, since: DateTime<Utc>) -> Result<UpdateResponse, ApiError> {
        let url = format!("{}/updates?locale=en", self.base);
        let body = serde_json::json!({
            "nudge": true,
            "last_activity_date": format_activity(since),
        });
        let resp = self
            .execute(self.http.post(&url).headers(self.headers()).json(&body))
            .await?;
        resp.json::<UpdateResponse>()
            .await
            .map_err(|e| ApiError::Decode(e.to_string()))
    }

    async fn get_user(&self, user_id: &str) -> Result<UserProfile, ApiError> {
        let url = format!("{}/user/{user_id}?locale=en", self.base);
        let resp = self
            .execute(self.http.get(&url).headers(self.headers()))
            .await?;
        let r: UserProfileResponse = resp
            .json()
            .await
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        if r.status >= 400 {
            return Err(status_error(r.status as u16, "Profile unavailable".into()));
        }
        r.results
            .ok_or_else(|| ApiError::Decode("no user in /user response".into()))
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<RefreshResult, ApiError> {
        let url = format!("{}/v3/auth/login?locale=en", self.base);
        let payload = proto::encode_refresh_request(refresh_token);

        let mut headers = self.headers();
        headers.insert(
            "Content-Type",
            "application/x-google-protobuf".parse().unwrap(),
        );
        headers.insert("is-created-as-guest", "false".parse().unwrap());

        let resp = self
            .execute(self.http.post(&url).headers(headers).body(payload))
            .await?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ApiError::Network(e.without_url().to_string()))?;
        let result = proto::decode_refresh_response(&bytes).map_err(ApiError::Decode)?;

        // Persist the new tokens in memory.
        if let Ok(mut st) = self.auth.write() {
            st.auth_token = Some(result.auth_token.clone());
            if let Some(rt) = &result.refresh_token
                && !rt.is_empty()
            {
                st.refresh_token = Some(rt.clone());
            }
            if let Some(uid) = &result.user_id {
                st.user_id = Some(uid.clone());
            }
        }
        Ok(result)
    }

    async fn download_photo(&self, url: &str) -> Result<Vec<u8>, ApiError> {
        let url = url::Url::parse(url).map_err(|e| ApiError::Other(e.to_string()))?;
        if url.scheme() != "https" {
            return Err(ApiError::Other("photo URL must use HTTPS".into()));
        }
        // CDN requests must never carry account credentials.
        let resp = self.execute(self.http.get(url)).await?;
        if resp.content_length().is_some_and(|n| n > 20 * 1024 * 1024) {
            return Err(ApiError::Other("photo exceeds 20 MB".into()));
        }
        let mut resp = resp;
        let mut bytes = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| ApiError::Network(e.without_url().to_string()))?
        {
            if bytes.len() + chunk.len() > 20 * 1024 * 1024 {
                return Err(ApiError::Other("photo exceeds 20 MB".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

fn status_error(code: u16, body: String) -> ApiError {
    match code {
        401 => ApiError::Auth,
        429 => ApiError::RateLimited,
        _ => ApiError::Status { code, body },
    }
}
fn envelope_data<T>(env: Envelope<T>) -> Result<T, ApiError> {
    if let Some(code) = env
        .meta
        .as_ref()
        .and_then(|v| v.get("status"))
        .and_then(|v| v.as_u64())
        .filter(|n| *n >= 400)
    {
        return Err(status_error(code as u16, "Request rejected".into()));
    }
    env.data
        .ok_or_else(|| ApiError::Decode("response has no data".into()))
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;

/// Some mutations return 204, others an envelope. Never treat a logical error as success.
async fn mutation_ack(response: reqwest::Response) -> Result<(), ApiError> {
    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(());
    }
    let v: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ApiError::Decode(e.to_string()))?;
    check_mutation(&v)?;
    let status = v
        .pointer("/meta/status")
        .or_else(|| v.get("status"))
        .and_then(|v| v.as_u64());
    if status.is_some_and(|s| (200..300).contains(&s))
        || v.get("success").and_then(|v| v.as_bool()) == Some(true)
        || v.pointer("/data/user").is_some_and(|v| v.is_object())
    {
        Ok(())
    } else {
        Err(ApiError::Decode(
            "The server did not confirm the change. Check Tinder before retrying.".into(),
        ))
    }
}
fn check_mutation(v: &serde_json::Value) -> Result<(), ApiError> {
    if let Some(code) = v
        .pointer("/meta/status")
        .or_else(|| v.get("status"))
        .and_then(|n| n.as_u64())
        .filter(|n| *n >= 400)
    {
        return Err(status_error(code as u16, "Request rejected".into()));
    }
    if v.get("error")
        .is_some_and(|e| !e.is_null() && e != &serde_json::Value::Bool(false))
        || v.get("success").and_then(|s| s.as_bool()) == Some(false)
    {
        return Err(ApiError::Other(
            "Request rejected by Tinder. No local changes were applied.".into(),
        ));
    }
    if !v.is_object() {
        return Err(ApiError::Decode("Unexpected acknowledgement".into()));
    }
    Ok(())
}
