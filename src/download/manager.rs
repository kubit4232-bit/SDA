//! Менеджер задач загрузки: реальный прогресс, пауза/возобновление,
//! лимит скорости и ограничение числа одновременных загрузок.

use super::control::JobControl;
use super::job::{DownloadJob, JobInfo, JobStatus};
use super::state::DownloadConfig;
use crate::network::HttpClient;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, RwLock};
use tracing::{debug, info};
use uuid::Uuid;

/// Ограничитель одновременных загрузок с динамическим лимитом.
struct Gate {
    active: AtomicUsize,
    limit: AtomicUsize,
    notify: Notify,
}

impl Gate {
    fn new(limit: usize) -> Self {
        Self {
            active: AtomicUsize::new(0),
            limit: AtomicUsize::new(limit.max(1)),
            notify: Notify::new(),
        }
    }

    async fn acquire(self: &Arc<Self>) -> GateGuard {
        loop {
            let active = self.active.load(Ordering::SeqCst);
            if active < self.limit.load(Ordering::SeqCst)
                && self
                    .active
                    .compare_exchange(active, active + 1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                return GateGuard {
                    gate: Arc::clone(self),
                };
            }
            let _ = tokio::time::timeout(Duration::from_millis(250), self.notify.notified()).await;
        }
    }

    fn set_limit(&self, limit: usize) {
        self.limit.store(limit.max(1), Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

struct GateGuard {
    gate: Arc<Gate>,
}

impl Drop for GateGuard {
    fn drop(&mut self) {
        self.gate.active.fetch_sub(1, Ordering::SeqCst);
        self.gate.notify.notify_waiters();
    }
}

/// Менеджер активных задач загрузки
pub struct DownloadManager {
    config: Arc<RwLock<DownloadConfig>>,
    client: HttpClient,
    jobs: Arc<RwLock<Vec<JobInfo>>>,
    handles: Arc<RwLock<HashMap<Uuid, tokio::task::JoinHandle<Result<()>>>>>,
    controls: Arc<RwLock<HashMap<Uuid, Arc<JobControl>>>>,
    gate: Arc<Gate>,
    state_path: PathBuf,
}

impl DownloadManager {
    pub fn new(config: DownloadConfig, client: HttpClient) -> Self {
        Self::with_state_dir(config, client, state_file())
    }

    /// Создать менеджер с явным путём реестра задач (для тестов/портативных сборок).
    pub fn with_state_dir(config: DownloadConfig, client: HttpClient, state_path: PathBuf) -> Self {
        let gate = Arc::new(Gate::new(config.max_concurrent_downloads));
        let mut loaded = load_jobs(&state_path);
        for job in &mut loaded {
            if matches!(job.status, JobStatus::Downloading | JobStatus::Queued) {
                // После перезапуска незавершённые задачи ждут команды пользователя.
                job.status = JobStatus::Paused;
                job.speed_bps = 0;
            }
        }
        Self {
            config: Arc::new(RwLock::new(config)),
            client,
            jobs: Arc::new(RwLock::new(loaded)),
            handles: Arc::new(RwLock::new(HashMap::new())),
            controls: Arc::new(RwLock::new(HashMap::new())),
            gate,
            state_path,
        }
    }

    /// Актуальная конфигурация.
    pub async fn config(&self) -> DownloadConfig {
        self.config.read().await.clone()
    }

    /// Применить новые настройки (сегменты, повторы, лимиты).
    pub async fn update_config(&self, config: DownloadConfig) {
        self.gate.set_limit(config.max_concurrent_downloads);
        let limit = config.max_speed_bps.unwrap_or(0);
        let controls = self.controls.read().await;
        for control in controls.values() {
            control.set_max_speed(limit);
        }
        *self.config.write().await = config;
    }

    /// Глобальный лимит скорости: None или Some(0) — без ограничения.
    pub async fn set_speed_limit(&self, max_speed_bps: Option<u64>) {
        let limit = max_speed_bps.unwrap_or(0);
        {
            let mut config = self.config.write().await;
            config.max_speed_bps = if limit == 0 { None } else { Some(limit) };
        }
        let controls = self.controls.read().await;
        for control in controls.values() {
            control.set_max_speed(limit);
        }
    }

    /// Добавить новую задачу в очередь
    pub async fn add_job(&self, job: DownloadJob) -> Result<Uuid> {
        let id = job.id;
        let info = JobInfo {
            id,
            url: job.url.clone(),
            path: job.save_path.clone(),
            status: JobStatus::Queued,
            total_bytes: 0,
            downloaded_bytes: 0,
            speed_bps: 0,
            segment_count: job.segment_count,
            username: job.username.clone(),
            password: job.password.clone(),
            cookies: job.cookies.clone(),
        };

        {
            let mut jobs = self.jobs.write().await;
            jobs.push(info);
        }

        self.spawn_job(job.clone()).await;

        debug!("Job {} queued: {}", id, job.url);
        self.persist().await;
        Ok(id)
    }

    /// Запустить задачу (контроль уже создан или создаётся заново).
    /// Заменить HTTP-клиент (например, после изменения прокси в настройках).
    pub fn set_client(&mut self, client: HttpClient) {
        self.client = client;
    }

    async fn spawn_job(&self, job: DownloadJob) {
        let id = job.id;
        let control = Arc::new(JobControl::new(job.max_speed_bps));
        {
            let mut controls = self.controls.write().await;
            controls.insert(id, Arc::clone(&control));
        }

        let inner = DownloadManagerInner {
            config: Arc::clone(&self.config),
            client: self.client.clone(),
            jobs: Arc::clone(&self.jobs),
            gate: Arc::clone(&self.gate),
            state_path: self.state_path.clone(),
        };
        let handle = tokio::spawn(async move { inner.run_job(job, control).await });

        let mut handles = self.handles.write().await;
        handles.insert(id, handle);
    }

    /// Получить список всех задач (с актуальным прогрессом).
    pub async fn list_jobs(&self) -> Vec<JobInfo> {
        let controls = self.controls.read().await.clone();
        let mut jobs = self.jobs.write().await;
        for job in jobs.iter_mut() {
            if let Some(control) = controls.get(&job.id) {
                job.total_bytes = control.total();
                job.downloaded_bytes = control.downloaded();
                job.speed_bps = if control.is_paused() { 0 } else { control.speed() };
            }
        }
        jobs.clone()
    }

    /// Получить задачу по ID
    pub async fn get_job(&self, id: Uuid) -> Option<JobInfo> {
        let jobs = self.list_jobs().await;
        jobs.into_iter().find(|j| j.id == id)
    }

    /// Приостановить задачу (активную или ожидающую).
    pub async fn pause_job(&self, id: Uuid) -> Result<()> {
        if let Some(control) = self.controls.read().await.get(&id).cloned() {
            control.set_paused(true);
        }
        {
            let mut jobs = self.jobs.write().await;
            let job = jobs
                .iter_mut()
                .find(|j| j.id == id)
                .context(format!("Задача {} не найдена", id))?;
            match job.status {
                JobStatus::Downloading | JobStatus::Queued => {
                    job.status = JobStatus::Paused;
                    job.speed_bps = 0;
                }
                _ => anyhow::bail!("Задача {} не может быть поставлена на паузу", id),
            }
        }
        self.persist().await;
        Ok(())
    }

    /// Возобновить приостановленную задачу (в т.ч. после перезапуска).
    pub async fn resume_job(&self, id: Uuid) -> Result<()> {
        let control = self.controls.read().await.get(&id).cloned();
        if let Some(control) = control {
            {
                let mut jobs = self.jobs.write().await;
                let job = jobs
                    .iter_mut()
                    .find(|j| j.id == id)
                    .context(format!("Задача {} не найдена", id))?;
                if !matches!(job.status, JobStatus::Paused | JobStatus::Queued) {
                    anyhow::bail!("Задача {} не на паузе", id);
                }
                job.status = JobStatus::Downloading;
            }
            control.set_paused(false);
        } else {
            let limit = self.config.read().await.max_speed_bps;
            let download_job = {
                let mut jobs = self.jobs.write().await;
                let job = jobs
                    .iter_mut()
                    .find(|j| j.id == id)
                    .context(format!("Задача {} не найдена", id))?;
                if matches!(job.status, JobStatus::Completed) {
                    anyhow::bail!("Задача {} уже завершена", id);
                }
                job.status = JobStatus::Queued;
                job.downloaded_bytes = 0;
                job.speed_bps = 0;
                DownloadJob {
                    id: job.id,
                    url: job.url.clone(),
                    save_path: job.path.clone(),
                    segment_count: job.segment_count.max(1),
                    max_speed_bps: limit,
                    username: job.username.clone(),
                    password: job.password.clone(),
                    cookies: job.cookies.clone(),
                }
            };
            self.spawn_job(download_job).await;
        }
        self.persist().await;
        Ok(())
    }

    /// Отменить задачу и остановить её потоки.
    pub async fn cancel_job(&self, id: Uuid) -> Result<()> {
        if let Some(control) = self.controls.read().await.get(&id).cloned() {
            control.cancel();
        }
        {
            let mut handles = self.handles.write().await;
            if let Some(handle) = handles.remove(&id) {
                handle.abort();
            }
        }
        {
            let mut jobs = self.jobs.write().await;
            if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
                job.status = JobStatus::Cancelled;
                job.speed_bps = 0;
            }
        }
        self.persist().await;
        Ok(())
    }

    /// Удалить задачу из списка (файл на диске остаётся).
    pub async fn remove_job(&self, id: Uuid) -> Result<()> {
        self.cancel_job(id).await?;
        {
            let mut jobs = self.jobs.write().await;
            jobs.retain(|j| j.id != id);
        }
        {
            let mut controls = self.controls.write().await;
            controls.remove(&id);
        }
        self.persist().await;
        Ok(())
    }

    /// Массовые операции.
    pub async fn pause_all(&self) {
        let ids: Vec<Uuid> = self.jobs.read().await.iter().map(|j| j.id).collect();
        for id in ids {
            let _ = self.pause_job(id).await;
        }
    }

    pub async fn resume_all(&self) {
        let ids: Vec<Uuid> = self
            .jobs
            .read()
            .await
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Paused))
            .map(|j| j.id)
            .collect();
        for id in ids {
            let _ = self.resume_job(id).await;
        }
    }

    pub async fn stop_all(&self) {
        let ids: Vec<Uuid> = self.jobs.read().await.iter().map(|j| j.id).collect();
        for id in ids {
            let _ = self.cancel_job(id).await;
        }
    }

    /// Суммарная скорость всех активных загрузок.
    pub async fn total_speed(&self) -> u64 {
        self.controls
            .read()
            .await
            .values()
            .filter(|c| !c.is_paused())
            .map(|c| c.speed())
            .sum()
    }

    /// Сохранить список задач на диск.
    async fn persist(&self) {
        persist_jobs(&self.jobs, &self.state_path).await;
    }
}

#[derive(Clone)]
struct DownloadManagerInner {
    config: Arc<RwLock<DownloadConfig>>,
    client: HttpClient,
    jobs: Arc<RwLock<Vec<JobInfo>>>,
    gate: Arc<Gate>,
    state_path: PathBuf,
}

impl DownloadManagerInner {
    async fn run_job(&self, job: DownloadJob, control: Arc<JobControl>) -> Result<()> {
        let id = job.id;
        let _guard = self.gate.acquire().await;

        if control.is_cancelled() {
            return Ok(());
        }

        let config = self.config.read().await.clone();
        if config.max_speed_bps.is_some() {
            control.set_max_speed(config.max_speed_bps.unwrap_or(0));
        }

        {
            let mut jobs = self.jobs.write().await;
            if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
                j.status = if control.is_paused() {
                    JobStatus::Paused
                } else {
                    JobStatus::Downloading
                };
            }
        }
        info!("Starting job {}: {}", id, job.url);

        let result = super::engine::download_file(&self.client, &job, &config, Arc::clone(&control)).await;

        let mut jobs = self.jobs.write().await;
        if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
            if control.is_cancelled() {
                j.status = JobStatus::Cancelled;
            } else {
                j.total_bytes = control.total();
                match &result {
                    Ok(()) => {
                        j.status = JobStatus::Completed;
                        j.downloaded_bytes = if control.total() > 0 {
                            control.total()
                        } else {
                            control.downloaded()
                        };
                    }
                    Err(e) => {
                        if e.to_string().contains(super::engine::CANCELLED) {
                            j.status = JobStatus::Cancelled;
                        } else {
                            j.status = JobStatus::Failed {
                                message: e.to_string(),
                            };
                        }
                    }
                }
            }
            j.speed_bps = 0;
        }
        drop(jobs);
        persist_jobs(&self.jobs, &self.state_path).await;

        if control.is_cancelled() || result.as_ref().err().is_some_and(|e| e.to_string().contains(super::engine::CANCELLED)) {
            return Ok(());
        }
        result.context(format!("Download job {} failed", id))
    }
}

impl Clone for DownloadManager {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            client: self.client.clone(),
            jobs: Arc::clone(&self.jobs),
            handles: Arc::clone(&self.handles),
            controls: Arc::clone(&self.controls),
            gate: Arc::clone(&self.gate),
            state_path: self.state_path.clone(),
        }
    }
}

/// Файл реестра задач (как список IDM между запусками).
fn state_file() -> PathBuf {
    crate::download::state::portable_data_dir().join("jobs.json")
}

fn load_jobs(path: &PathBuf) -> Vec<JobInfo> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
        .unwrap_or_default()
}

fn save_jobs(path: &PathBuf, jobs: &[JobInfo]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(jobs) {
        let _ = std::fs::write(path, json);
    }
}

async fn persist_jobs(jobs: &Arc<RwLock<Vec<JobInfo>>>, path: &PathBuf) {
    let snapshot = jobs.read().await.clone();
    save_jobs(path, &snapshot);
}
