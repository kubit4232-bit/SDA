//! Модуль извлечения прямых ссылок на видео через yt-dlp.
//! Поддерживает YouTube, YouTube Shorts, TikTok, Instagram, Facebook, Vimeo и многие другие.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use tracing::{debug, info};

/// Информация о видео, извлечённая через yt-dlp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    pub url: String,
    pub title: Option<String>,
    pub duration: Option<u64>,
    pub ext: Option<String>,
    pub filesize: Option<u64>,
}

/// Результат извлечения ссылки
#[derive(Debug)]
pub struct ExtractedVideo {
    pub direct_url: String,
    pub title: Option<String>,
    pub filename: String,
}

/// Проверяет, является ли URL видеохостингом, поддерживаемым yt-dlp
pub fn is_video_url(url: &str) -> bool {
    let video_hosts = [
        "youtube.com",
        "youtu.be",
        "youtube-nocookie.com",
        "tiktok.com",
        "vm.tiktok.com",
        "instagram.com",
        "facebook.com",
        "fb.watch",
        "vimeo.com",
        "dailymotion.com",
        "twitch.tv",
        "twitter.com",
        "x.com",
        "reddit.com",
        "streamable.com",
        "clipchamp.com",
    ];

    let url_lower = url.to_lowercase();
    video_hosts.iter().any(|host| url_lower.contains(host))
}

/// Поиск yt-dlp executable: ищет рядом с текущим exe, затем в PATH
fn find_ytdlp() -> Option<String> {
    // Сначала ищем рядом с текущим executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let ytdlp_path = dir.join("yt-dlp.exe");
            if ytdlp_path.exists() {
                return Some(ytdlp_path.to_string_lossy().to_string());
            }
        }
    }

    // Затем через which (PATH)
    if let Ok(output) = std::process::Command::new("where").arg("yt-dlp").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().next() {
                return Some(line.trim().to_string());
            }
        }
    }

    None
}

/// Извлекает прямую ссылку на видео через yt-dlp
pub async fn extract_video_url(video_url: &str) -> Result<ExtractedVideo> {
    let ytdlp_path = find_ytdlp()
        .context("yt-dlp не найден. Скачайте yt-dlp.exe и положите рядом с SDA")?;

    info!("Используем yt-dlp: {}", ytdlp_path);

    // Запускаем yt-dlp для получения информации о видео
    let output = Command::new(&ytdlp_path)
        .args([
            "--get-url",
            "--get-title",
            "--get-filename",
            "--no-warnings",
            video_url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Не удалось запустить yt-dlp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("yt-dlp ошибка: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.is_empty() {
        anyhow::bail!("yt-dlp не вернул ссылку на видео");
    }

    // Находим первую строку, которая является URL (начинается с http)
    let direct_url = lines
        .iter()
        .find(|line| line.trim().starts_with("http"))
        .map(|s| s.trim().to_string())
        .context("yt-dlp не вернул прямую ссылку на видео")?;

    // Находим первую строку, которая НЕ является URL (это название)
    let title = lines
        .iter()
        .find(|line| !line.trim().starts_with("http"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    debug!("yt-dlp direct URL: {}", direct_url);
    debug!("yt-dlp title: {:?}", title);

    Ok(ExtractedVideo {
        direct_url,
        title: title.clone(),
        filename: title.clone().unwrap_or_else(|| "video".to_string()),
    })
}

/// Извлекает ссылку на видео и возвращает VideoInfo
pub async fn extract_video_info(video_url: &str) -> Result<VideoInfo> {
    let extracted = extract_video_url(video_url).await?;

    Ok(VideoInfo {
        url: extracted.direct_url,
        title: extracted.title,
        duration: None,
        ext: extracted.filename.split('.').last().map(|s| s.to_string()),
        filesize: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_video_url() {
        assert!(is_video_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(is_video_url("https://youtu.be/dQw4w9WgXcQ"));
        assert!(is_video_url("https://www.youtube.com/shorts/dB32Pd8wblw"));
        assert!(is_video_url("https://tiktok.com/@user/video/123"));
        assert!(is_video_url("https://vimeo.com/123456"));
        assert!(!is_video_url("https://example.com/file.zip"));
        assert!(!is_video_url("https://google.com"));
    }
}
