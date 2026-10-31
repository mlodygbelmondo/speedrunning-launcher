//! The on-disk cache of API answers, and what the UI gets back: the value, when it was fetched,
//! and why it could not be refreshed, so the page can say "updated 3 h ago, offline" instead of
//! showing nothing.
use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{Refresh, http::ApiError};

/// `Refresh::Now` still reuses anything younger than this: the APIs cache a few seconds anyway,
/// and a refresh button pressed five times must not cost five rounds of requests.
pub const MIN_REFRESH: Duration = Duration::from_secs(10);

/// How far in the future a fetch time may be before the value counts as stale (`Cached::is_fresh`).
pub const CLOCK_SLACK: Duration = Duration::from_secs(5);

/// The cached value at `file` while it is fresh for `refresh` (within `ttl`), else the result of
/// `fetch`, stored on success. When `fetch` fails, the stale cached value comes back with
/// `refresh_error` set, except for `NotFound`, which is never hidden behind a cache: the name is
/// wrong or was changed. `Refresh::Never` returns the cache, or `Offline`, without fetching; a
/// cached value past its TTL then comes back with `refresh_error: Some(NotRefreshed)`, so a Home
/// tile can't show days-old numbers as current.
pub(crate) async fn cached_fetch<T>(
    file: &Path,
    ttl: Duration,
    refresh: Refresh,
    fetch: impl Future<Output = Result<T, ApiError>>,
) -> Result<Cached<T>, ApiError>
where
    T: Serialize + DeserializeOwned + Sync,
{
    let cached: Option<Cached<T>> = load(file).await;
    if refresh == Refresh::Never {
        let mut c = cached.ok_or(ApiError::Offline("not fetched yet".into()))?;
        if !c.is_fresh(ttl) {
            c.refresh_error = Some(ApiError::NotRefreshed);
        }
        return Ok(c);
    }
    let keep = match refresh {
        Refresh::Now => MIN_REFRESH.min(ttl),
        _ => ttl,
    };
    let cached = match cached {
        Some(c) if c.is_fresh(keep) => return Ok(c),
        other => other,
    };
    match fetch.await {
        Ok(value) => {
            let c = Cached::new(value);
            store(file, &c).await;
            Ok(c)
        }
        Err(ApiError::NotFound) => Err(ApiError::NotFound),
        Err(e) => match cached {
            Some(mut c) => {
                c.refresh_error = Some(e);
                Ok(c)
            }
            None => Err(e),
        },
    }
}

/// A value from an API with its age.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cached<T> {
    pub value: T,
    /// Epoch ms of the successful fetch this value comes from.
    pub fetched_at_ms: i64,
    /// Set when `value` is older than its TTL: this call tried to refresh and failed, or it was a
    /// cache-only call (`ApiError::NotRefreshed`). `None` when the value is within its TTL.
    #[serde(skip)]
    pub refresh_error: Option<ApiError>,
}

impl<T> Cached<T> {
    pub fn new(value: T) -> Self {
        Cached {
            value,
            fetched_at_ms: now_ms(),
            refresh_error: None,
        }
    }

    pub fn age(&self) -> Duration {
        Duration::from_millis(now_ms().saturating_sub(self.fetched_at_ms).max(0) as u64)
    }

    /// True when the value is within `ttl` of its fetch. A fetch time in the future (more than
    /// `CLOCK_SLACK` ahead) counts as stale: the clock was wrong at the fetch or is now (a
    /// dual-boot PC whose RTC runs on local time until NTP corrects it), so the age is unknown,
    /// and treating it as "just now" would make the refresh button do nothing for hours.
    pub fn is_fresh(&self, ttl: Duration) -> bool {
        self.fetched_at_ms <= now_ms() + CLOCK_SLACK.as_millis() as i64 && self.age() < ttl
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Cached<U> {
        Cached {
            value: f(self.value),
            fetched_at_ms: self.fetched_at_ms,
            refresh_error: self.refresh_error,
        }
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Writes through a temporary file and a rename, so a crash or a second launcher never sees half
/// a file. The temporary name is unique per process and per call: two writers of the same file
/// in one process (two cache stores of one API part) must not write into each other's temp file.
/// On Windows the rename can fail while another writer replaces the same file ("Access denied");
/// every caller treats a failed write as a cache update that did not happen.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    tmp.push(format!(".{}.{n}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Reads a cached value; `None` if missing or unreadable (a broken cache only means a refetch).
pub async fn load<T: DeserializeOwned>(path: &Path) -> Option<Cached<T>> {
    let bytes = tokio::fs::read(path).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub async fn store<T: Serialize + Sync>(path: &Path, value: &Cached<T>) {
    let Ok(bytes) = serde_json::to_vec(value) else {
        return;
    };
    let path = path.to_path_buf();
    // Small files; spawn_blocking keeps the rename and fsync-free write off the async threads.
    let _ = tokio::task::spawn_blocking(move || write_atomic(&path, &bytes)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn round_trip_and_age() {
        let dir = std::env::temp_dir().join(format!("srl-stats-cache-{}", std::process::id()));
        let file = dir.join("a/b.json");
        let mut c = Cached::new(vec![1, 2, 3]);
        c.fetched_at_ms -= 90_000;
        store(&file, &c).await;
        let back: Cached<Vec<i32>> = load(&file).await.unwrap();
        assert_eq!(back.value, vec![1, 2, 3]);
        assert!(back.age() >= Duration::from_secs(90));
        assert!(!back.is_fresh(Duration::from_secs(60)));
        assert!(back.is_fresh(Duration::from_secs(600)));
        std::fs::write(&file, b"garbage").unwrap();
        assert!(load::<Vec<i32>>(&file).await.is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn stale_cache_when_the_fetch_fails() {
        let dir = std::env::temp_dir().join(format!("srl-stats-fetch-{}", std::process::id()));
        let file = dir.join("x.json");
        let ttl = Duration::from_secs(60);
        let offline = || async { Err::<u32, _>(ApiError::Offline("down".into())) };
        // Nothing cached: the error comes through; Never does not fetch.
        assert!(
            cached_fetch(&file, ttl, Refresh::IfStale, offline())
                .await
                .is_err()
        );
        assert!(
            cached_fetch(&file, ttl, Refresh::Never, async { Ok(1u32) })
                .await
                .is_err()
        );
        // Fetched once, then fresh: the next fetch is never polled.
        let c = cached_fetch(&file, ttl, Refresh::IfStale, async { Ok(7u32) })
            .await
            .unwrap();
        assert_eq!((c.value, c.refresh_error), (7, None));
        let c = cached_fetch(&file, ttl, Refresh::IfStale, async { Ok(8u32) })
            .await
            .unwrap();
        assert_eq!(c.value, 7);
        // Refresh::Now within MIN_REFRESH still uses the cache.
        let c = cached_fetch(&file, ttl, Refresh::Now, async { Ok(9u32) })
            .await
            .unwrap();
        assert_eq!(c.value, 7);
        // Stale and offline: the old value, marked, with its real age.
        let mut old: Cached<u32> = load(&file).await.unwrap();
        old.fetched_at_ms -= 3_600_000;
        store(&file, &old).await;
        let c = cached_fetch(&file, ttl, Refresh::IfStale, offline())
            .await
            .unwrap();
        assert_eq!(c.value, 7);
        assert!(c.age() >= Duration::from_secs(3600));
        assert!(matches!(c.refresh_error, Some(ApiError::Offline(_))));
        // NotFound is never hidden.
        let nf = cached_fetch(&file, ttl, Refresh::IfStale, async {
            Err::<u32, _>(ApiError::NotFound)
        });
        assert_eq!(nf.await.unwrap_err(), ApiError::NotFound);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn cache_only_marks_an_old_value() {
        let dir = std::env::temp_dir().join(format!("srl-stats-never-{}", std::process::id()));
        let file = dir.join("x.json");
        let ttl = Duration::from_secs(120);
        let unused = || async { Ok::<u32, ApiError>(1) };
        store(&file, &Cached::new(5u32)).await;
        // Within the TTL: no mark.
        let c = cached_fetch(&file, ttl, Refresh::Never, unused())
            .await
            .unwrap();
        assert_eq!((c.value, c.refresh_error), (5, None));
        // Three days old: same value, marked, with its real age.
        let mut old: Cached<u32> = load(&file).await.unwrap();
        old.fetched_at_ms -= 3 * 86_400_000;
        store(&file, &old).await;
        let c = cached_fetch(&file, ttl, Refresh::Never, unused())
            .await
            .unwrap();
        assert_eq!(c.value, 5);
        assert_eq!(c.refresh_error, Some(ApiError::NotRefreshed));
        assert!(c.age() >= Duration::from_secs(3 * 86_400));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_fetch_time_in_the_future_is_stale() {
        let dir = std::env::temp_dir().join(format!("srl-stats-future-{}", std::process::id()));
        let file = dir.join("x.json");
        let ttl = Duration::from_secs(120);
        let mut c = Cached::new(1u32);
        c.fetched_at_ms += 2 * 3_600_000;
        assert!(!c.is_fresh(ttl));
        store(&file, &c).await;
        // IfStale and Now both fetch again.
        let got = cached_fetch(&file, ttl, Refresh::Now, async { Ok(2u32) })
            .await
            .unwrap();
        assert_eq!(got.value, 2);
        assert!(got.fetched_at_ms <= now_ms());
        // A couple of seconds ahead (two clocks, rounding) is still fresh.
        let mut near = Cached::new(3u32);
        near.fetched_at_ms += 2000;
        assert!(near.is_fresh(ttl));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn concurrent_atomic_writes_of_one_file_do_not_collide() {
        let dir = std::env::temp_dir().join(format!("srl-stats-atomic-{}", std::process::id()));
        let file = dir.join("same.json");
        let handles: Vec<_> = (0..8u8)
            .map(|i| {
                let file = file.clone();
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        let r = write_atomic(&file, &vec![b'a' + i; 64 * 1024]);
                        // Windows refuses a rename onto a file another rename is replacing
                        // ("Access denied", seen under wine); callers ignore a failed cache
                        // write. Anything else must succeed.
                        if cfg!(not(windows)) {
                            r.unwrap();
                        }
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        // Whatever won, the file is one writer's whole content.
        let bytes = std::fs::read(&file).unwrap();
        assert_eq!(bytes.len(), 64 * 1024);
        assert!(bytes.iter().all(|&b| b == bytes[0]));
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            1,
            "no temp files left"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
