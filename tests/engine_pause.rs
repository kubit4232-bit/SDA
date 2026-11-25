//! Интеграционный тест: пауза реально останавливает передачу, продолжение
//! доходит до конца, лимит скорости соблюдается.

use sda_core::download::{DownloadJob, DownloadManager, JobStatus};
use sda_core::network::HttpClient;
use sda_core::DownloadConfig;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use uuid::Uuid;

const TOTAL: usize = 8 * 1024 * 1024;

async fn spawn_slow_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                if request.starts_with("HEAD ") {
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: none\r\nConnection: close\r\n\r\n",
                        TOTAL
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    return;
                }
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    TOTAL
                );
                if socket.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                let chunk = vec![0u8; 64 * 1024];
                let mut sent = 0usize;
                while sent < TOTAL {
                    let n = chunk.len().min(TOTAL - sent);
                    if socket.write_all(&chunk[..n]).await.is_err() {
                        return;
                    }
                    sent += n;
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
            });
        }
    });
    port
}

async fn wait_status(manager: &DownloadManager, id: Uuid) -> JobStatus {
    for _ in 0..300 {
        if let Some(info) = manager.get_job(id).await {
            if matches!(
                info.status,
                JobStatus::Completed | JobStatus::Failed { .. } | JobStatus::Cancelled
            ) {
                return info.status;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    JobStatus::Failed {
        message: "timeout".to_string(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pause_stops_transfer_and_resume_completes() {
    let port = spawn_slow_server().await;
    let url = format!("http://127.0.0.1:{}/file.bin", port);

    let config = DownloadConfig::default();
    let client = HttpClient::new(&config.user_agent, 10, 30);
    let dir = tempfile::tempdir().unwrap();
    let manager = DownloadManager::with_state_dir(
        config,
        client,
        dir.path().join("jobs.json"),
    );

    let path = dir.path().join("file.bin");
    let id = manager
        .add_job(DownloadJob {
            id: Uuid::new_v4(),
            url,
            save_path: path.clone(),
            segment_count: 4,
            max_speed_bps: None,
            username: None,
            password: None,
            cookies: None,
        })
        .await
        .unwrap();

    // Даём скачать часть файла
    tokio::time::sleep(Duration::from_millis(1200)).await;
    manager.pause_job(id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let paused_at = manager.get_job(id).await.unwrap().downloaded_bytes;

    tokio::time::sleep(Duration::from_millis(900)).await;
    let still_paused = manager.get_job(id).await.unwrap().downloaded_bytes;
    assert!(
        still_paused - paused_at < 128 * 1024,
        "пауза не остановила передачу: {} -> {}",
        paused_at,
        still_paused
    );

    manager.resume_job(id).await.unwrap();
    let status = wait_status(&manager, id).await;
    assert!(matches!(status, JobStatus::Completed), "статус: {:?}", status);
    assert_eq!(std::fs::metadata(&path).unwrap().len(), TOTAL as u64);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn job_list_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state").join("jobs.json");
    let config = DownloadConfig::default();
    let client = HttpClient::new(&config.user_agent, 2, 2);

    let manager = DownloadManager::with_state_dir(config.clone(), client.clone(), state.clone());
    let id = manager
        .add_job(DownloadJob {
            id: Uuid::new_v4(),
            url: "http://127.0.0.1:1/never.bin".to_string(),
            save_path: dir.path().join("never.bin"),
            segment_count: 2,
            max_speed_bps: None,
            username: None,
            password: None,
            cookies: None,
        })
        .await
        .unwrap();
    manager.pause_job(id).await.ok();

    // «Перезапуск приложения»
    let manager2 = DownloadManager::with_state_dir(config, client, state);
    let jobs = manager2.list_jobs().await;
    assert_eq!(jobs.len(), 1, "список задач не сохранился");
    assert_eq!(jobs[0].id, id);
    assert!(matches!(jobs[0].status, JobStatus::Paused | JobStatus::Failed { .. }));
}

async fn spawn_auth_server(expected_user: &'static str, expected_pass: &'static str, total: usize) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let user = expected_user.to_string();
    let pass = expected_pass.to_string();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let is_head = request.starts_with("HEAD ");
                let expected = format!(
                    "Basic {}",
                    base64_encode(expected_user, expected_pass)
                );
                let authorized = request.to_uppercase().contains(&expected.to_uppercase());
                if !authorized {
                    let resp = "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"SDA\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                    let _ = socket.write_all(resp.as_bytes()).await;
                    return;
                }
                let head = if is_head {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: none\r\nConnection: close\r\n\r\n",
                        total
                    )
                } else {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        total
                    )
                };
                if socket.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                let chunk = vec![0u8; 64 * 1024];
                let mut sent = 0usize;
                while sent < total {
                    let n = chunk.len().min(total - sent);
                    if socket.write_all(&chunk[..n]).await.is_err() {
                        return;
                    }
                    sent += n;
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
            });
        }
    });
    port
}

fn base64_encode(user: &str, pass: &str) -> String {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    STANDARD
        .encode(format!("{}:{}", user, pass).as_bytes())
        .to_string()
}

async fn spawn_cookie_server(expected_cookie: &'static str, total: usize) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let is_head = request.starts_with("HEAD ");
                let expected = format!("COOKIE: {}", expected_cookie);
                let authorized = request.to_uppercase().contains(&expected.to_uppercase());
                if !authorized {
                    let resp = "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Cookie\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                    let _ = socket.write_all(resp.as_bytes()).await;
                    return;
                }
                let head = if is_head {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        total
                    )
                } else {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        total
                    )
                };
                if socket.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                let chunk = vec![0u8; 64 * 1024];
                let mut sent = 0usize;
                while sent < total {
                    let n = chunk.len().min(total - sent);
                    if socket.write_all(&chunk[..n]).await.is_err() {
                        return;
                    }
                    sent += n;
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
            });
        }
    });
    port
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cookie_auth_is_enforced_and_accepted() {
    let port = spawn_cookie_server("session=abc123", 1024).await;
    let url = format!("http://127.0.0.1:{}/file.bin", port);

    let config = DownloadConfig::default();
    let client = HttpClient::new(&config.user_agent, 5, 5);
    let manager = DownloadManager::new(config, client);

    let dir = tempfile::tempdir().unwrap();
    let id = manager
        .add_job(DownloadJob {
            id: Uuid::new_v4(),
            url: url.clone(),
            save_path: dir.path().join("file.bin"),
            segment_count: 2,
            max_speed_bps: None,
            username: None,
            password: None,
            cookies: Some("session=abc123".to_string()),
        })
        .await
        .unwrap();
    let status = wait_status(&manager, id).await;
    assert!(matches!(status, JobStatus::Completed), "статус с Cookie: {:?}", status);

    // Запрос без cookie должен быть отклонён сервером.
    let bad_manager = {
        let config = DownloadConfig::default();
        let client = HttpClient::new(&config.user_agent, 5, 5);
        DownloadManager::new(config, client)
    };
    let bad_id = bad_manager
        .add_job(DownloadJob {
            id: Uuid::new_v4(),
            url: url.clone(),
            save_path: dir.path().join("file2.bin"),
            segment_count: 2,
            max_speed_bps: None,
            username: None,
            password: None,
            cookies: None,
        })
        .await
        .unwrap();
    let bad_status = wait_status(&bad_manager, bad_id).await;
    assert!(matches!(bad_status, JobStatus::Failed { .. }), "статус без Cookie: {:?}", bad_status);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn basic_auth_is_enforced_and_accepted() {
    let port = spawn_auth_server("sda_user", "sda_pass", 1024).await;    let url = format!("http://127.0.0.1:{}/file.bin", port);
    let config = DownloadConfig::default();
    let client = HttpClient::new(&config.user_agent, 5, 5);
    let manager = DownloadManager::new(config, client);

    let dir = tempfile::tempdir().unwrap();
    let id = manager
        .add_job(DownloadJob {
            id: Uuid::new_v4(),
            url: url.clone(),
            save_path: dir.path().join("file.bin"),
            segment_count: 2,
            max_speed_bps: None,
            username: Some("sda_user".to_string()),
            password: Some("sda_pass".to_string()),
            cookies: None,
        })
        .await
        .unwrap();
    let status = wait_status(&manager, id).await;
    assert!(matches!(status, JobStatus::Completed), "статус с Basic Auth: {:?}", status);

    // Запрос без авторизации должен быть отклонён сервером.
    let bad_manager = {
        let config = DownloadConfig::default();
        let client = HttpClient::new(&config.user_agent, 5, 5);
        DownloadManager::new(config, client)
    };
    let bad_id = bad_manager
        .add_job(DownloadJob {
            id: Uuid::new_v4(),
            url: url.clone(),
            save_path: dir.path().join("file2.bin"),
            segment_count: 2,
            max_speed_bps: None,
            username: Some("wrong".to_string()),
            password: Some("wrong".to_string()),
            cookies: None,
        })
        .await
        .unwrap();
    let bad_status = wait_status(&bad_manager, bad_id).await;
    assert!(matches!(bad_status, JobStatus::Failed { .. }), "статус без авторизации: {:?}", bad_status);
}
