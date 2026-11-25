//! Разделяемый контроль одной задачи: прогресс, скорость, пауза, отмена.
//!
//! Используется движком (пишет прогресс) и менеджером/GUI (читает и управляет).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct SpeedSample {
    last_time: Instant,
    last_bytes: u64,
}

/// Потокобезопасное состояние одной загрузки.
pub struct JobControl {
    total: AtomicU64,
    downloaded: AtomicU64,
    speed: AtomicU64,
    paused: AtomicBool,
    cancelled: AtomicBool,
    max_speed: AtomicU64,
    sample: Mutex<SpeedSample>,
}

impl JobControl {
    pub fn new(max_speed_bps: Option<u64>) -> Self {
        Self {
            total: AtomicU64::new(0),
            downloaded: AtomicU64::new(0),
            speed: AtomicU64::new(0),
            paused: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            max_speed: AtomicU64::new(max_speed_bps.unwrap_or(0)),
            sample: Mutex::new(SpeedSample {
                last_time: Instant::now(),
                last_bytes: 0,
            }),
        }
    }

    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    pub fn set_total(&self, value: u64) {
        self.total.store(value, Ordering::Relaxed);
    }

    pub fn downloaded(&self) -> u64 {
        self.downloaded.load(Ordering::Relaxed)
    }

    pub fn reset_downloaded(&self) {
        self.downloaded.store(0, Ordering::Relaxed);
    }

    /// Учесть принятые байты и пересчитать скорость (окно ~500 мс).
    pub fn add_downloaded(&self, bytes: u64) {
        let total = self.downloaded.fetch_add(bytes, Ordering::Relaxed) + bytes;
        if let Ok(mut sample) = self.sample.lock() {
            let elapsed = sample.last_time.elapsed();
            if elapsed >= Duration::from_millis(500) {
                let delta = total.saturating_sub(sample.last_bytes);
                let bps = (delta as f64 / elapsed.as_secs_f64()) as u64;
                self.speed.store(bps, Ordering::Relaxed);
                sample.last_time = Instant::now();
                sample.last_bytes = total;
            }
        }
    }

    pub fn speed(&self) -> u64 {
        self.speed.load(Ordering::Relaxed)
    }

    pub fn set_speed(&self, bps: u64) {
        self.speed.store(bps, Ordering::Relaxed);
    }

    /// Лимит скорости в байт/с; 0 — без ограничения.
    pub fn max_speed(&self) -> u64 {
        self.max_speed.load(Ordering::Relaxed)
    }

    pub fn set_max_speed(&self, bps: u64) {
        self.max_speed.store(bps, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, value: bool) {
        self.paused.store(value, Ordering::Relaxed);
        if value {
            self.speed.store(0, Ordering::Relaxed);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Дождаться снятия паузы. Возвращает false, если задачу отменили.
    pub async fn wait_if_paused(&self) -> bool {
        while self.is_paused() {
            if self.is_cancelled() {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        !self.is_cancelled()
    }
}
