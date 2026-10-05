use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

fn server(
    responses: Vec<(u16, Vec<u8>)>,
) -> (RealApi, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                    let len = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .and_then(|s| s.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            tx.send(String::from_utf8_lossy(&request).into_owned())
                .unwrap();
            write!(stream,"HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    let config = crate::config::Config {
        auth_token: Some("test-auth".into()),
        device_id: Some("test-device".into()),
        ..Default::default()
    };
    let mut api = RealApi::new(&config).unwrap();
    api.base = format!("http://{address}");
    (api, rx, handle)
}
fn json(s: &str) -> (u16, Vec<u8>) {
    (200, s.as_bytes().to_vec())
}
#[tokio::test]
async fn profile_headers_and_identity_are_validated() {
    let (api, rx, h) = server(vec![json(
        r#"{"meta":{"status":200},"data":{"user":{"_id":"own","name":"Test"}}}"#,
    )]);
    assert_eq!(api.get_own_user().await.unwrap().id, "own");
    let req = rx.recv().unwrap().to_ascii_lowercase();
    assert!(req.contains("x-auth-token: test-auth"));
    assert!(req.contains("persistent-device-id: test-device"));
    assert!(req.starts_with("get /v2/profile?"));
    h.join().unwrap();
}
#[tokio::test]
async fn message_cursor_is_opaque_and_query_encoded() {
    let (api, rx, h) = server(vec![json(
        r#"{"data":{"messages":[{"_id":"one","message":"hey","sent_date":"2026-09-18T12:00:00Z"}],"next_page_token":"server-next"}}"#,
    )]);
    let page = api
        .get_messages("match", 50, Some("a+/=&?".into()))
        .await
        .unwrap();
    assert_eq!(page.next_page_token.as_deref(), Some("server-next"));
    assert!(page.messages[0].time().is_some());
    let request = rx.recv().unwrap();
    let target = request.split_whitespace().nth(1).unwrap();
    let url = url::Url::parse(&format!("http://test{target}")).unwrap();
    assert_eq!(
        url.query_pairs()
            .find(|(k, _)| k == "page_token")
            .unwrap()
            .1,
        "a+/=&?"
    );
    h.join().unwrap();
}
#[tokio::test]
async fn matches_follow_server_pagination_without_duplicate_rows() {
    let (api, rx, h) = server(vec![
        json(r#"{"data":{"matches":[{"_id":"a"}],"next_page_token":"page+/="}}"#),
        json(r#"{"data":{"matches":[{"_id":"a"},{"_id":"b"}]}}"#),
    ]);
    let matches = api.get_matches(10, false).await.unwrap();
    assert_eq!(matches.len(), 2);
    rx.recv().unwrap();
    let second = rx.recv().unwrap();
    assert!(second.contains("page_token=page%2B%2F%3D"));
    h.join().unwrap();
}
#[tokio::test]
async fn swipe_uses_post_like_get_pass_and_preserves_recommendation_metadata() {
    let (api, rx, h) = server(vec![
        json(r#"{"match":false,"likes_remaining":4}"#),
        json(r#"{"status":200}"#),
    ]);
    let rec = Recommendation {
        user: UserProfile {
            id: "person".into(),
            photos: vec![super::super::types::Photo {
                id: "photo".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
        s_number: Some(123.into()),
        ..Default::default()
    };
    let result = api.swipe(&rec, true).await.unwrap();
    assert!(!result.matched);
    assert_eq!(result.likes_remaining, Some(4));
    let like = rx.recv().unwrap();
    assert!(like.starts_with("POST /like/person?"));
    let body: serde_json::Value =
        serde_json::from_str(like.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["s_number"], 123);
    assert_eq!(body["liked_content_id"], "photo");
    api.swipe(&rec, false).await.unwrap();
    let pass = rx.recv().unwrap();
    assert!(pass.starts_with("GET /pass/person?"));
    assert!(pass.contains("s_number=123"));
    h.join().unwrap();
}
#[tokio::test]
async fn send_requires_server_acknowledgement_and_accepts_id_alias() {
    let (api, rx, h) = server(vec![
        json(r#"{"id":"server-message","message":"hello"}"#),
        json(r#"{"error":"not sent"}"#),
    ]);
    let msg = api
        .send_message("own", "other", "match", "hello")
        .await
        .unwrap();
    assert_eq!(msg.id, "server-message");
    assert_eq!(msg.from.as_deref(), Some("own"));
    rx.recv().unwrap();
    assert!(
        api.send_message("own", "other", "match", "hello")
            .await
            .is_err()
    );
    h.join().unwrap();
}
#[tokio::test]
async fn http_and_envelope_errors_are_not_successful_empty_lists() {
    let (api, _rx, h) = server(vec![
        (401, vec![]),
        (429, vec![]),
        json(r#"{"meta":{"status":401},"data":{"matches":[]}}"#),
        json(r#"{}"#),
    ]);
    assert!(matches!(
        api.get_matches(20, false).await,
        Err(ApiError::Auth)
    ));
    assert!(matches!(
        api.get_matches(20, false).await,
        Err(ApiError::RateLimited)
    ));
    assert!(matches!(
        api.get_matches(20, false).await,
        Err(ApiError::Auth)
    ));
    assert!(matches!(
        api.get_matches(20, false).await,
        Err(ApiError::Decode(_))
    ));
    h.join().unwrap();
}
#[tokio::test]
async fn discovery_maps_distance_and_embedded_swipe_errors_keep_failure() {
    let (api, _rx, h) = server(vec![
        json(
            r#"{"data":{"results":[{"user":{"_id":"p","name":"Test"},"distance_mi":12,"s_number":99}]}}"#,
        ),
        json(r#"{"status":429,"error":"limit"}"#),
    ]);
    let recs = api.get_recommendations().await.unwrap();
    assert_eq!(recs[0].user.distance_mi, Some(12));
    assert!(api.swipe(&recs[0], true).await.is_err());
    h.join().unwrap();
}
#[tokio::test]
async fn real_photo_loader_rejects_local_paths() {
    let api = RealApi::new(&crate::config::Config::default()).unwrap();
    assert!(api.download_photo("/etc/passwd").await.is_err());
    assert!(api.download_photo("http://localhost/photo").await.is_err());
}

#[tokio::test]
async fn refresh_roundtrip_updates_in_memory_credentials() {
    let login = vec![0x0a, 1, b'r', 0x12, 3, b'n', b'e', b'w'];
    let mut reply = vec![0x42, login.len() as u8];
    reply.extend(login);
    let (api, rx, h) = server(vec![
        (200, reply),
        json(r#"{"data":{"user":{"_id":"own"}}}"#),
    ]);
    let result = api.refresh_token("refresh-test").await.unwrap();
    assert_eq!(result.auth_token, "new");
    let request = rx.recv().unwrap();
    assert!(request.starts_with("POST /v3/auth/login?"));
    assert!(request.contains("application/x-google-protobuf"));
    api.get_own_user().await.unwrap();
    assert!(rx.recv().unwrap().contains("x-auth-token: new"));
    h.join().unwrap();
}

#[tokio::test]
async fn account_patch_is_partial_and_unmatch_uses_delete() {
    let (api, rx, h) = server(vec![
        json(r#"{"meta":{"status":200},"data":{"user":{"bio":"hello"}}}"#),
        (204, vec![]),
    ]);
    api.update_profile(&ProfileUpdate {
        bio: Some("hello".into()),
        ..Default::default()
    })
    .await
    .unwrap();
    api.unmatch("match/with?chars").await.unwrap();
    let request = rx.recv().unwrap();
    assert!(request.starts_with("POST /v2/profile?locale=en "));
    let body: serde_json::Value =
        serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body, serde_json::json!({"user":{"bio":"hello"}}));
    assert!(
        rx.recv()
            .unwrap()
            .starts_with("DELETE /user/matches/match%2Fwith%3Fchars ")
    );
    h.join().unwrap();
}

#[tokio::test]
async fn mutations_reject_logical_errors_and_unknown_acknowledgements() {
    let (api, _rx, h) = server(vec![
        json(r#"{"meta":{"status":403}}"#),
        json(r#"{"error":"blocked"}"#),
        json(r#"{"success":false}"#),
        json("null"),
        json("{}"),
    ]);
    assert!(api.update_profile(&ProfileUpdate::default()).await.is_err());
    for _ in 0..4 {
        assert!(api.unmatch("test").await.is_err());
    }
    h.join().unwrap();
}

#[tokio::test]
async fn super_like_metadata_and_boost_confirmation_are_checked() {
    let (api, rx, h) = server(vec![
        json(r#"{"status":200,"match":false,"super_likes":{"remaining":2}}"#),
        json(r#"{"boost_id":"boost-1","remaining":1}"#),
        json(r#"{"remaining":1}"#),
    ]);
    let rec = Recommendation {
        user: UserProfile {
            id: "person".into(),
            photos: vec![super::super::types::Photo {
                id: "photo-1".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
        s_number: Some(serde_json::json!(123)),
        ..Default::default()
    };
    let (result, remaining) = api.super_like(&rec).await.unwrap();
    assert!(!result.matched);
    assert_eq!(remaining, Some(2));
    let req = rx.recv().unwrap();
    assert!(req.starts_with("POST /like/person/super?locale=en "));
    let body: serde_json::Value =
        serde_json::from_str(req.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(
        body,
        serde_json::json!({"s_number":123,"liked_content_id":"photo-1","liked_content_type":"photo"})
    );
    assert_eq!(api.boost().await.unwrap(), Some(1));
    assert!(rx.recv().unwrap().starts_with("POST /boost?locale=en "));
    assert!(api.boost().await.is_err());
    h.join().unwrap();
}
