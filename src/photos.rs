//! Asynchronous photo pipeline: downloads (or reads) raw bytes through the
//! active API backend, caches them on disk, decodes and downscales them into
//! display previews while retaining full-detail pixels, and reports readiness so the
//! UI can redraw when an image arrives.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use crate::api::client::TinderApi;
use crate::images::{self, RenderedImage};

/// Render size class. `Avatar` is the small square used in the match list;
/// `Photo` is the large image used in the profile carousel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SizeClass {
    Avatar,
    Photo,
}

/// An image became available (or failed) for the given key (`"{url}@{class}"`).
#[derive(Debug, Clone)]
pub struct PhotoReady {
    pub key: String,
    pub img: Option<Arc<RenderedImage>>,
    pub error: Option<String>,
}

pub struct PhotoPipeline {
    pub graphics: crate::graphics::Graphics,
    api: Arc<dyn TinderApi>,
    disk: PathBuf,
    mem: Arc<Mutex<HashMap<String, Arc<RenderedImage>>>>,
    inflight: Arc<Mutex<HashSet<String>>>,
    tx: mpsc::UnboundedSender<PhotoReady>,
    sizes: Mutex<HashMap<SizeClass, (u16, u16)>>,
    failures: Arc<Mutex<HashMap<String, Instant>>>,
    generation: Arc<AtomicU64>,
    permits: Arc<tokio::sync::Semaphore>,
}

fn key_of(url: &str, class: SizeClass) -> String {
    let c = match class {
        SizeClass::Avatar => "avatar",
        SizeClass::Photo => "photo",
    };
    format!("{url}@{c}")
}

/// FNV-1a hex digest, good enough for stable cache filenames.
fn hash_hex(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

impl PhotoPipeline {
    pub fn new(
        api: Arc<dyn TinderApi>,
        disk_dir: PathBuf,
        tx: mpsc::UnboundedSender<PhotoReady>,
    ) -> Self {
        let _ = std::fs::create_dir_all(&disk_dir);
        let _ = crate::cache::prune(&disk_dir, false);
        Self {
            graphics: crate::graphics::Graphics::default(),
            api,
            disk: disk_dir,
            mem: Arc::new(Mutex::new(HashMap::new())),
            inflight: Arc::new(Mutex::new(HashSet::new())),
            tx,
            failures: Arc::new(Mutex::new(HashMap::new())),
            generation: Arc::new(AtomicU64::new(0)),
            permits: Arc::new(tokio::sync::Semaphore::new(4)),
            sizes: Mutex::new(HashMap::from([
                (SizeClass::Avatar, (10, 10)),
                (SizeClass::Photo, (46, 26)),
            ])),
        }
    }

    pub fn set_sizes(&self, avatar: u16, photo_w: u16, photo_h: u16) {
        let mut s = self.sizes.lock().unwrap();
        s.insert(SizeClass::Avatar, (avatar, avatar));
        s.insert(SizeClass::Photo, (photo_w, photo_h));
    }

    /// Synchronous lookup used while drawing; returns the image if it has
    /// already been rendered.
    fn key(&self, url: &str, class: SizeClass) -> String {
        let (w, h) = self.sizes.lock().unwrap()[&class];
        format!(
            "{}@{w}x{h}@{}",
            key_of(url, class),
            self.generation.load(Ordering::Relaxed)
        )
    }
    pub fn get(&self, url: &str, class: SizeClass) -> Option<Arc<RenderedImage>> {
        self.mem.lock().unwrap().get(&self.key(url, class)).cloned()
    }
    pub fn failed(&self, url: &str, class: SizeClass) -> bool {
        self.failures
            .lock()
            .unwrap()
            .contains_key(&self.key(url, class))
    }

    #[cfg(test)]
    pub fn is_ready(&self, url: &str, class: SizeClass) -> bool {
        self.get(url, class).is_some()
    }

    /// Ensure `url` is downloaded and rendered at `class` size. No-op when the
    /// image is already rendered or a download for this key is running.
    pub fn request(&self, url: &str, class: SizeClass) {
        if url.trim().is_empty() {
            return;
        }
        let key = self.key(url, class);
        if self
            .failures
            .lock()
            .unwrap()
            .get(&key)
            .is_some_and(|t| t.elapsed() < Duration::from_secs(30))
        {
            return;
        }
        if self.mem.lock().unwrap().contains_key(&key) {
            return;
        }
        {
            let mut inf = self.inflight.lock().unwrap();
            if !inf.insert(key.clone()) {
                return;
            }
        }

        let (w, h) = self.sizes.lock().unwrap()[&class];
        let api = Arc::clone(&self.api);
        let disk = self.disk.clone();
        let tx = self.tx.clone();
        let mem = Arc::clone(&self.mem);
        let key_owned = key.clone();
        let url_owned = url.to_string();
        let inflight = self.inflight.clone();
        let failures = self.failures.clone();
        let permits = self.permits.clone();
        let generation = self.generation.clone();
        let started_generation = generation.load(Ordering::Relaxed);

        tokio::spawn(async move {
            let Ok(_permit) = permits.acquire_owned().await else {
                return;
            };
            let finished = run_load(&api, &disk, &url_owned, w, h).await;
            inflight.lock().unwrap().remove(&key_owned);
            if started_generation != generation.load(Ordering::Relaxed) {
                return;
            }
            match finished {
                Ok(img) => {
                    {
                        let mut cache = mem.lock().unwrap();
                        while cache.len() >= 64
                            || cache.values().map(|i| i.source_bytes()).sum::<usize>()
                                + img.source_bytes()
                                > 128 * 1024 * 1024
                        {
                            if let Some(old) = cache.keys().next().cloned() {
                                cache.remove(&old);
                            } else {
                                break;
                            }
                        }
                        cache.insert(key_owned.clone(), Arc::clone(&img));
                    }
                    failures.lock().unwrap().remove(&key_owned);
                    let _ = tx.send(PhotoReady {
                        key: key_owned,
                        img: Some(img),
                        error: None,
                    });
                }
                Err(e) => {
                    failures
                        .lock()
                        .unwrap()
                        .insert(key_owned.clone(), Instant::now());
                    let _ = tx.send(PhotoReady {
                        key: key_owned,
                        img: None,
                        error: Some(e),
                    });
                }
            }
        });
    }

    /// Drop all rendered images (used on sign-out).
    pub fn clear(&self) {
        self.graphics.clear();
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.mem.lock().unwrap().clear();
        self.failures.lock().unwrap().clear();
    }
}

async fn run_load(
    api: &Arc<dyn TinderApi>,
    disk: &std::path::Path,
    url: &str,
    w: u16,
    h: u16,
) -> Result<Arc<RenderedImage>, String> {
    let cache_path = disk.join(format!("{}.bin", hash_hex(url)));
    let cached = std::fs::read(&cache_path).ok();
    if let Some(bytes) = cached
        && let Some(img) = tokio::task::spawn_blocking(move || images::render(&bytes, w, h))
            .await
            .map_err(|e| e.to_string())?
    {
        return Ok(Arc::new(img));
    }
    let bytes = api
        .download_photo(url)
        .await
        .map_err(|e| format!("download: {e}"))?;
    tokio::task::spawn_blocking(move || {
        let img =
            images::render(&bytes, w, h).ok_or_else(|| "image decode/resize failed".to_string())?;
        let temp = cache_path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let write = (|| -> std::io::Result<()> {
            use std::io::Write;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.open(&temp)?.write_all(&bytes)
        })();
        if write.is_ok() {
            let _ = std::fs::rename(&temp, &cache_path);
        }
        let _ = std::fs::remove_file(temp);
        if let Some(dir) = cache_path.parent() {
            let _ = crate::cache::prune(dir, false);
        }
        Ok(Arc::new(img))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::mock::MockApi;

    #[tokio::test]
    async fn pipeline_caches_and_notifies() {
        let api: Arc<dyn TinderApi> = Arc::new(MockApi::new(&Config::default()));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let dir = std::env::temp_dir().join(format!("ttui-pipeline-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let pipe = PhotoPipeline::new(Arc::clone(&api), dir.clone(), tx);

        // Pick a real mock photo URL.
        let own = api.get_own_user().await.unwrap();
        let url = own.photos[0].display_url().unwrap();

        assert!(!pipe.is_ready(&url, SizeClass::Photo));
        pipe.request(&url, SizeClass::Photo);

        let ready = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            ready.img.is_some(),
            "expected image, got err {:?}",
            ready.error
        );
        assert!(pipe.is_ready(&url, SizeClass::Photo));

        // Second request is a no-op (no duplicate ready event).
        pipe.request(&url, SizeClass::Photo);
        assert!(rx.try_recv().is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn pipeline_reports_failure_for_garbage_url() {
        let api: Arc<dyn TinderApi> = Arc::new(MockApi::new(&Config::default()));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let dir = std::env::temp_dir().join(format!("ttui-pipeline-test2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let pipe = PhotoPipeline::new(api, dir.clone(), tx);
        pipe.request("/nonexistent/photo.png", SizeClass::Photo);
        let ready = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(ready.img.is_none());
        assert!(ready.error.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use crate::{config::Config, mock::MockApi};
    #[tokio::test]
    async fn clear_resize_and_corrupt_disk_cache_recover() {
        let api: Arc<dyn TinderApi> = Arc::new(MockApi::new(&Config::default()));
        let url = api.get_own_user().await.unwrap().photos[0]
            .display_url()
            .unwrap();
        let dir =
            std::env::temp_dir().join(format!("ttui-image-recovery-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let disk = dir.join(format!("{}.bin", hash_hex(&url)));
        std::fs::write(&disk, b"corrupt").unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let pipe = PhotoPipeline::new(api, dir.clone(), tx);
        pipe.request(&url, SizeClass::Photo);
        let first = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(first.img.is_some());
        pipe.clear();
        pipe.set_sizes(8, 20, 10);
        pipe.request(&url, SizeClass::Photo);
        let second = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let img = second.img.unwrap();
        assert!(img.width <= 20 && img.height <= 10);
        assert!(pipe.get(&url, SizeClass::Photo).is_some());
        assert_ne!(std::fs::read(&disk).unwrap(), b"corrupt");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
