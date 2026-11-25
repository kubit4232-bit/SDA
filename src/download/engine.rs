//! Основной движок загрузки: сегментированное скачивание с resume,
//! реальным прогрессом, паузой, отменой, лимитом скорости и повторами.
//! Поддержка YouTube и других видеохостингов через yt-dlp.

use super::control::JobControl;
use super::job::DownloadJob;
use super::state::DownloadConfig;
use crate::download::youtube_extractor::{extract_video_url, is_video_url};
use crate::network::HttpClient;
use anyhow::{anyhow, Context, Result};
use futures_util::stream::StreamExt;
use std::io::SeekFrom;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tracing::{debug, info, warn};

/// Признак отмены задачи (не ошибка сети).
pub const CANCELLED: &str = "SDA_TASK_CANCELLED";

/// Основная точка входа движка
pub async fn download_file(
    client: &HttpClient,
    job: &DownloadJob,
    config: &DownloadConfig,
    control: Arc<JobControl>,
) -> Result<()> {
    let id = job.id;
    let original_url = job.url.clone();
    let path = job.save_path.clone();
    let segment_count = config
        .connection_exceptions
        .iter()
        .find(|e| !e.site.is_empty() && original_url.contains(&e.site))
        .map(|e| e.connections.max(1))
        .unwrap_or_else(|| {
            let cap = match config.connection_type.as_str() {
                "medium" => 4,
                "low" => 1,
                _ => usize::MAX,
            };
            job.segment_count.max(1).min(cap).max(1)
        });
    let client = client
        .clone()
        .with_basic_auth(job.username.as_deref(), job.password.as_deref())
        .with_cookie_header(job.cookies.as_deref());

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }

    // Определяем, является ли это видеохостингом
    let is_video = is_video_url(&original_url);

    let download_url = if is_video {
        info!("Job {}: video URL, извлекаем прямую ссылку через yt-dlp: {}", id, original_url);
        let extracted = extract_video_url(&original_url)
            .await
            .context("Не удалось извлечь ссылку на видео через yt-dlp")?;
        info!("Job {}: yt-dlp вернул прямую ссылку: {}", id, extracted.direct_url);
        if let Some(title) = &extracted.title {
            info!("Job {}: название видео: {}", id, title);
        }
        extracted.direct_url
    } else {
        original_url.clone()
    };

    if !control.wait_if_paused().await {
        return Err(anyhow!(CANCELLED));
    }

    // HEAD-запрос для получения размера и проверки поддержки Range
    let head_response = match client.head(&download_url).await {
        Ok(r) => Some(r),
        Err(e) => {
            let err_str = format!("{:?}", e);
            if err_str.contains("builder") {
                warn!("Job {}: URL builder error - URL may contain unsupported characters", id);
                let url_display: String = download_url.chars().take(100).collect();
                return Err(anyhow!(
                    "Не удалось создать HTTP запрос к URL (builder error). URL: {}",
                    url_display
                ));
            }
            warn!("Job {}: HEAD request failed: {:?}", id, e);
            None
        }
    };

    let total_size: u64 = head_response
        .as_ref()
        .and_then(|r| r.headers().get("content-length"))
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let accepts_ranges = head_response
        .as_ref()
        .and_then(|r| r.headers().get("accept-ranges"))
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("bytes"))
        .unwrap_or(false);

    control.set_total(total_size);
    info!(
        "Job {}: total_size={}, accepts_ranges={}, segments={}",
        id, total_size, accepts_ranges, segment_count
    );

    // Если файл уже полностью загружен
    if path.exists() {
        let existing = path.metadata()?.len();
        if total_size > 0 && existing == total_size {
            info!("Job {}: file already complete", id);
            control.set_total(total_size);
            control.add_downloaded(total_size.saturating_sub(control.downloaded()));
            return Ok(());
        }
    }

    if total_size > 0 && accepts_ranges && segment_count > 1 {
        download_segmented(
            client,
            &download_url,
            &path,
            total_size,
            segment_count,
            config.max_retries,
            Arc::clone(&control),
        )
        .await
    } else {
        download_single(client, &download_url, &path, total_size, config.max_retries, &control).await
    }
}

/// Сегментированная загрузка (несколько параллельных потоков через Range)
async fn download_segmented(
    client: HttpClient,
    url: &str,
    path: &Path,
    total_size: u64,
    segment_count: usize,
    max_retries: u32,
    control: Arc<JobControl>,
) -> Result<()> {
    let segment_size = (total_size / segment_count as u64).max(1);
    let mut handles = Vec::new();

    for i in 0..segment_count {
        let start = i as u64 * segment_size;
        if start >= total_size {
            break;
        }
        let end = if i == segment_count - 1 {
            total_size - 1
        } else {
            (start + segment_size - 1).min(total_size - 1)
        };

        let client = client.clone();
        let url = url.to_string();
        let path = path.to_path_buf();
        let segment_id = i as u32;
        let control = Arc::clone(&control);

        handles.push(tokio::spawn(async move {
            download_range_with_retry(
                &client,
                &url,
                &path,
                segment_id,
                start,
                end,
                max_retries,
                segment_count,
                control,
            )
            .await
        }));
    }

    for handle in handles {
        handle.await??;
    }

    Ok(())
}

/// Загрузка диапазона с повторами и докачкой при обрыве.
#[allow(clippy::too_many_arguments)]
async fn download_range_with_retry(
    client: &HttpClient,
    url: &str,
    path: &Path,
    segment_index: u32,
    start: u64,
    end: u64,
    max_retries: u32,
    segment_count: usize,
    control: Arc<JobControl>,
) -> Result<()> {
    let mut offset = start;
    let mut segment_written = 0u64;
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 0..=max_retries {
        if control.is_cancelled() {
            return Err(anyhow!(CANCELLED));
        }
        if !control.wait_if_paused().await {
            return Err(anyhow!(CANCELLED));
        }

        if offset > end {
            return Ok(());
        }

        match download_range_once(
            client,
            url,
            path,
            segment_index,
            offset,
            end,
            segment_count,
            &control,
            &mut segment_written,
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(e) => {
                if control.is_cancelled() {
                    return Err(anyhow!(CANCELLED));
                }
                let text = e.to_string();
                if text.contains(CANCELLED) {
                    return Err(e);
                }
                warn!(
                    "Segment {} attempt {}/{} failed: {}",
                    segment_index,
                    attempt + 1,
                    max_retries + 1,
                    text
                );
                // Продолжаем с места, где остановились (счётчик сегмента).
                offset = start + segment_written.min(end - start + 1);
                last_error = Some(e);
                if attempt < max_retries {
                    tokio::time::sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("Segment {} failed", segment_index)))
}

async fn download_range_once(
    client: &HttpClient,
    url: &str,
    path: &Path,
    segment_index: u32,
    start: u64,
    end: u64,
    segment_count: usize,
    control: &JobControl,
    segment_written: &mut u64,
) -> Result<()> {
    let range = format!("bytes={}-{}", start, end);
    debug!("Segment {}: Range {}", segment_index, range);

    let response = client.get_ranged(url, &range).await?;
    let mut stream = response.bytes_stream();

    let mut file = tokio::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
        .await
        .context(format!("Не удалось открыть файл для записи: {}", path.display()))?;
    file.seek(SeekFrom::Start(start)).await?;

    let mut segment_baseline = 0u64;
    let mut segment_last = Instant::now();

    while let Some(chunk) = stream.next().await {
        if control.is_cancelled() {
            return Err(anyhow!(CANCELLED));
        }
        if !control.wait_if_paused().await {
            return Err(anyhow!(CANCELLED));
        }

        let chunk = chunk?;
        file.write_all(&chunk).await?;
        *segment_written += chunk.len() as u64;
        control.add_downloaded(chunk.len() as u64);

        let limit = control.max_speed();
        if limit > 0 {
            // Делим общий лимит между сегментами (агрегат ≈ заданному лимиту).
            let seg_limit = (limit / segment_count as u64).max(1);
            let bytes_in_window = *segment_written - segment_baseline;
            let expected = Duration::from_secs_f64(bytes_in_window as f64 / seg_limit as f64);
            let elapsed = segment_last.elapsed();
            if expected > elapsed {
                tokio::time::sleep(expected - elapsed).await;
            } else if elapsed >= Duration::from_secs(1) {
                segment_baseline = *segment_written;
                segment_last = Instant::now();
            }
        }
    }

    file.flush().await?;
    debug!("Segment {}: completed ({}-{})", segment_index, start, end);
    Ok(())
}

/// Одиночная загрузка (без сегментирования)
async fn download_single(
    client: HttpClient,
    url: &str,
    path: &Path,
    _total_size: u64,
    max_retries: u32,
    control: &JobControl,
) -> Result<()> {
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 0..=max_retries {
        if control.is_cancelled() {
            return Err(anyhow!(CANCELLED));
        }
        if !control.wait_if_paused().await {
            return Err(anyhow!(CANCELLED));
        }

        let response = client.get(url).await?;
        let mut stream = response.bytes_stream();

        let mut file = File::create(path).await?;

        let started = Instant::now();
        let mut written = 0u64;
        let mut failed: Option<anyhow::Error> = None;

        while let Some(chunk) = stream.next().await {
            if control.is_cancelled() {
                return Err(anyhow!(CANCELLED));
            }
            if !control.wait_if_paused().await {
                return Err(anyhow!(CANCELLED));
            }
            match chunk {
                Ok(bytes) => {
                    file.write_all(&bytes).await?;
                    written += bytes.len() as u64;
                    control.add_downloaded(bytes.len() as u64);

                    let limit = control.max_speed();
                    if limit > 0 {
                        let expected = Duration::from_secs_f64(written as f64 / limit as f64);
                        let elapsed = started.elapsed();
                        if expected > elapsed {
                            tokio::time::sleep(expected - elapsed).await;
                        }
                    }
                }
                Err(e) => {
                    failed = Some(e.into());
                    break;
                }
            }
        }

        file.flush().await?;

        match failed {
            None => {
                info!("Single download completed ({} bytes)", written);
                return Ok(());
            }
            Some(e) => {
                warn!(
                    "Single download attempt {}/{} interrupted: {}",
                    attempt + 1,
                    max_retries + 1,
                    e
                );
                last_error = Some(e);
                if attempt < max_retries {
                    tokio::time::sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("Single download failed")))
}
