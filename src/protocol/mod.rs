//! Протокол IPC между браузерным расширением, MsgHost и ядром SDA.
//! Формат analogous IDM: 4-байтная длина (LE) + JSON payload.

use serde::{Deserialize, Serialize};

/// Сообщения протокола SDA
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SdaMessage {
    /// Запрос на скачивание (из расширения в ядро)
    DownloadRequest {
        url: String,
        #[serde(default)]
        referer: Option<String>,
        #[serde(default)]
        cookies: Option<String>,
        #[serde(default)]
        save_path: Option<String>,
        #[serde(default)]
        username: Option<String>,
        #[serde(default)]
        password: Option<String>,
    },
    /// Подтверждение принятия задачи
    DownloadAccepted {
        job_id: String,
    },
    /// Обновление прогресса
    Progress {
        job_id: String,
        downloaded: u64,
        total: Option<u64>,
        speed_bps: u64,
    },
    /// Завершение загрузки
    Completed {
        job_id: String,
        path: String,
    },
    /// Ошибка
    Error {
        job_id: Option<String>,
        message: String,
    },
    /// Команда приостановить
    Pause {
        job_id: String,
    },
    /// Команда возобновить
    Resume {
        job_id: String,
    },
    /// Команда отменить
    Cancel {
        job_id: String,
    },
    /// Запрос списка активных задач
    ListJobs,
    /// Ответ со списком задач
    JobList {
        jobs: Vec<crate::download::JobInfo>,
    },
}

/// Фрейм IPC: длина + данные
#[derive(Debug)]
pub struct SdaFrame {
    pub length: u32,
    pub payload: Vec<u8>,
}

impl SdaFrame {
    pub fn encode(message: &SdaMessage) -> Result<Vec<u8>, serde_json::Error> {
        let payload = serde_json::to_vec(message)?;
        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(&payload);
        Ok(frame)
    }

    pub fn decode(buffer: &[u8]) -> Option<(SdaMessage, usize)> {
        if buffer.len() < 4 {
            return None;
        }
        let length = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
        if buffer.len() < 4 + length {
            return None;
        }
        let payload = &buffer[4..4 + length];
        let message = serde_json::from_slice(payload).ok()?;
        Some((message, 4 + length))
    }
}
