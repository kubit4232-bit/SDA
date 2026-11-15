//! Точка входа CLI для SDA Core.
//!
//! Использование:
//!   sda download <URL> [--output <path>] [--segments <n>]
//!   sda list
//!   sda status <job-id>
//!   sda serve          — запуск в режиме IPC (named pipe / stdio)

use clap::Parser;
use sda_core::{
    download::{DownloadJob, DownloadManager, JobStatus},
    network::HttpClient,
    protocol::{SdaFrame, SdaMessage},
    DownloadConfig,
};
use std::path::PathBuf;
use tracing::{error, info};
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, NAMED_PIPE_MODE,
    PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES,
};
use windows::core::PCWSTR;
use anyhow::Context;

const FRAME_HEADER_SIZE: usize = 4;

#[derive(Parser, Debug)]
#[command(author, version, about = "SDA - Super Download Agent")]
enum Args {
    /// Скачать файл
    Download {
        #[arg(help = "URL для скачивания")]
        url: String,

        #[arg(short = 'o', long = "output", help = "Путь для сохранения")]
        output: Option<PathBuf>,

        #[arg(
            short = 's',
            long = "segments",
            default_value = "8",
            help = "Количество сегментов"
        )]
        segments: usize,
    },
    /// Список активных задач
    List,
    /// Статус задачи
    Status {
        #[arg(help = "Идентификатор задачи")]
        job_id: String,
    },
    /// Запуск IPC-режима (Named Pipe для браузерного расширения / MsgHost)
    Serve {
        #[arg(
            long = "pipe-name",
            default_value = r"\\.\pipe\SDANetworkMonitor"
        )]
        pipe_name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let config = DownloadConfig::default();
    let client = HttpClient::new(
        &config.user_agent,
        config.connect_timeout_secs,
        config.read_timeout_secs,
    );
    let manager = DownloadManager::new(config.clone(), client);

    match args {
        Args::Download {
            url,
            output,
            segments,
        } => {
            let save_path = output.unwrap_or_else(|| {
                PathBuf::from(format!("./sda_download_{}", uuid::Uuid::new_v4()))
            });

            let job = DownloadJob {
                id: uuid::Uuid::new_v4(),
                url,
                save_path,
                segment_count: segments,
                max_speed_bps: config.max_speed_bps,
            };

            let job_id = manager.add_job(job).await?;
            info!("Download started, job_id={}", job_id);

            // Ждём завершения (простой CLI режим)
            loop {
                let job_info = manager.get_job(job_id).await;
                match job_info {
                    Some(info) => match info.status {
                        JobStatus::Completed => {
                            println!("Download completed: {:?}", info.path);
                            break;
                        }
                        JobStatus::Failed { message } => {
                            error!("Download failed: {}", message);
                            break;
                        }
                        _ => {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    },
                    None => break,
                }
            }
        }
        Args::List => {
            let jobs = manager.list_jobs().await;
            for job in jobs {
                println!(
                    "{} | {} | {:?} | {}/{} bytes",
                    job.id, job.url, job.status, job.downloaded_bytes, job.total_bytes
                );
            }
        }
        Args::Status { job_id } => {
            let id = uuid::Uuid::parse_str(&job_id)?;
            if let Some(info) = manager.get_job(id).await {
                println!("{:?}", info);
            } else {
                println!("Job not found");
            }
        }
        Args::Serve { pipe_name } => {
            info!("SDA IPC server starting on pipe: {}", pipe_name);
            serve_ipc(manager, &pipe_name).await?;
        }
    }

    Ok(())
}

/// Полноценный IPC-сервер на именованном канале Windows (аналог IDMNetMon pipe)
/// Реализует цикл: CreateNamedPipeW → ConnectNamedPipe → ReadFile/WriteFile → DisconnectNamedPipe
async fn serve_ipc(manager: DownloadManager, pipe_name: &str) -> anyhow::Result<()> {
    info!("SDA IPC server starting on pipe: {}", pipe_name);

    let mut pipe_name_wide: Vec<u16> = pipe_name.encode_utf16().collect();
    pipe_name_wide.push(0);
    let pipe_name_wide = pipe_name_wide.into_boxed_slice();

    loop {
        let pipe = unsafe {
            CreateNamedPipeW(
                windows::core::PCWSTR::from_raw(pipe_name_wide.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                NAMED_PIPE_MODE(PIPE_TYPE_MESSAGE.0 | PIPE_READMODE_MESSAGE.0),
                255, // max instances
                4096, // out buffer size
                4096, // in buffer size
                0, // default timeout
                None, // security attributes
            )
        };

        if pipe == INVALID_HANDLE_VALUE {
            anyhow::bail!("CreateNamedPipeW failed: invalid handle");
        }

        // Ждём подключения клиента (блокирующая операция в spawn_blocking)
        let pipe_for_connect = pipe;
        let result = tokio::task::spawn_blocking(move || {
            wait_for_client(pipe_for_connect)
        }).await?;

        let pipe = result?;

        // Обработка клиента в отдельной задаче
        let manager_clone = manager.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(manager_clone, pipe).await {
                error!("Client handler error: {}", e);
            }
        });
    }
}

/// Блокирующее ожидание подключения клиента к именованному каналу
fn wait_for_client(pipe: HANDLE) -> anyhow::Result<HANDLE> {
    unsafe { ConnectNamedPipe(pipe, None) }
        .map(|_| pipe)
        .map_err(|e| anyhow::anyhow!("ConnectNamedPipe failed: {}", e.code()))
}

/// Обработка одного клиентского подключения к Named Pipe
async fn handle_client(
    manager: DownloadManager,
    pipe: HANDLE,
) -> anyhow::Result<()> {
    info!("Client connected to Named Pipe");

    loop {
        // Читаем заголовок фрейма (4 байта длины)
        let len_bytes = read_exact_pipe(pipe, FRAME_HEADER_SIZE).await?;
        let length = u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;

        if length == 0 || length > 10_000_000 {
            error!("Invalid frame size: {}", length);
            break;
        }

        // Читаем полезную нагрузку фрейма
        let payload = read_exact_pipe(pipe, length).await?;

        // Декодируем сообщение
        let message: SdaMessage = serde_json::from_slice(&payload)
            .context("Failed to decode SdaMessage")?;

        // Обрабатываем сообщение и формируем ответ
        let response = process_message(&manager, message).await;

        // Кодируем и отправляем ответ
        let frame = SdaFrame::encode(&response)?;
        write_pipe_all(pipe, &frame).await?;
    }

    // Очистка при отключении клиента
    unsafe { DisconnectNamedPipe(pipe).ok() };
    unsafe { let _ = CloseHandle(pipe); };

    info!("Client disconnected from Named Pipe");
    Ok(())
}

/// Чтение точного количества байт из Named Pipe (синхронный ReadFile в spawn_blocking)
async fn read_exact_pipe(pipe: HANDLE, size: usize) -> anyhow::Result<Vec<u8>> {
    let pipe_clone = pipe;
    tokio::task::spawn_blocking(move || {
        let mut buf = vec![0u8; size];
        let mut offset = 0;

        while offset < size {
            let remaining = &mut buf[offset..];
            let mut read = 0u32;
            let result = unsafe {
                ReadFile(pipe_clone, Some(remaining), Some(&mut read), None)
            };
            if let Err(e) = result {
                return Err(anyhow::anyhow!("ReadFile failed: {}", e.code()));
            }
            if read == 0 {
                if offset == 0 {
                    return Err(anyhow::anyhow!("Pipe disconnected"));
                }
                break;
            }
            offset += read as usize;
        }

        buf.truncate(offset);
        Ok(buf)
    }).await?
}

/// Запись всех байт в Named Pipe (синхронный WriteFile в spawn_blocking)
async fn write_pipe_all(pipe: HANDLE, data: &[u8]) -> anyhow::Result<()> {
    let data_copy = data.to_vec();
    let pipe_clone = pipe;
    tokio::task::spawn_blocking(move || {
        let mut offset = 0;

        while offset < data_copy.len() {
            let remaining = &data_copy[offset..];
            let mut written = 0u32;
            let result = unsafe {
                WriteFile(pipe_clone, Some(remaining), Some(&mut written), None)
            };
            if let Err(e) = result {
                return Err(anyhow::anyhow!("WriteFile failed: {}", e.code()));
            }
            if written == 0 {
                return Err(anyhow::anyhow!("Pipe write returned 0 bytes"));
            }
            offset += written as usize;
        }

        Ok(())
    }).await?
}

/// Обработка входящего сообщения и формирование ответа
async fn process_message(manager: &DownloadManager, message: SdaMessage) -> SdaMessage {
    match message {
        SdaMessage::DownloadRequest { url, referer: _, cookies: _, save_path, .. } => {
            let id = uuid::Uuid::new_v4();
            let path = save_path.map(PathBuf::from).unwrap_or_else(|| {
                PathBuf::from(format!("./sda_download_{}", id))
            });

            let job = DownloadJob {
                id,
                url,
                save_path: path,
                segment_count: 8,
                max_speed_bps: None,
            };

            match manager.add_job(job).await {
                Ok(job_id) => {
                    info!("IPC: Download accepted, job_id={}", job_id);
                    SdaMessage::DownloadAccepted {
                        job_id: job_id.to_string(),
                    }
                }
                Err(e) => {
                    error!("IPC: Failed to add job: {}", e);
                    SdaMessage::Error {
                        job_id: None,
                        message: e.to_string(),
                    }
                }
            }
        }

        SdaMessage::Pause { job_id } => {
            let id = match uuid::Uuid::parse_str(&job_id) {
                Ok(id) => id,
                Err(e) => return SdaMessage::Error {
                    job_id: Some(job_id),
                    message: format!("Invalid job ID: {}", e),
                },
            };

            match manager.pause_job(id).await {
                Ok(()) => SdaMessage::DownloadAccepted { job_id },
                Err(e) => SdaMessage::Error {
                    job_id: Some(job_id),
                    message: e.to_string(),
                },
            }
        }

        SdaMessage::Resume { job_id } => {
            let id = match uuid::Uuid::parse_str(&job_id) {
                Ok(id) => id,
                Err(e) => return SdaMessage::Error {
                    job_id: Some(job_id),
                    message: format!("Invalid job ID: {}", e),
                },
            };

            match manager.resume_job(id).await {
                Ok(()) => SdaMessage::DownloadAccepted { job_id },
                Err(e) => SdaMessage::Error {
                    job_id: Some(job_id),
                    message: e.to_string(),
                },
            }
        }

        SdaMessage::Cancel { job_id } => {
            let id = match uuid::Uuid::parse_str(&job_id) {
                Ok(id) => id,
                Err(e) => return SdaMessage::Error {
                    job_id: Some(job_id),
                    message: format!("Invalid job ID: {}", e),
                },
            };

            match manager.cancel_job(id).await {
                Ok(()) => SdaMessage::DownloadAccepted { job_id },
                Err(e) => SdaMessage::Error {
                    job_id: Some(job_id),
                    message: e.to_string(),
                },
            }
        }

        SdaMessage::ListJobs => {
            let jobs = manager.list_jobs().await;
            SdaMessage::JobList { jobs }
        }

        other => {
            error!("IPC: Unknown message type: {:?}", other);
            SdaMessage::Error {
                job_id: None,
                message: "Unknown message type".to_string(),
            }
        }
    }
}
