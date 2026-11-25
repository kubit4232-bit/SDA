// SDA MsgHost — Native Messaging host для Chrome/Edge/Firefox.
// Читает сообщения из stdin, пересылает в SDA Core через Named Pipe
// `\\.\pipe\SDANetworkMonitor`, читает ответы из Named Pipe и пишет в stdout.
//
// Взаимодействие с Named Pipe — синхронное (CreateFileW / ReadFile / WriteFile /
// CloseHandle), выполняемое внутри `tokio::task::spawn_blocking`, чтобы
// избежать проблем со временем жизни OVERLAPPED/HANDLE (см. шаблон в src/main.rs).

use anyhow::{Context, Result};
use sda_core::protocol::{SdaFrame, SdaMessage};
use std::io::{self, Read, Write};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING, ReadFile, WriteFile,
};
use windows::core::PCWSTR;

const PIPE_NAME: &str = r"\\.\pipe\SDANetworkMonitor";
const FRAME_HEADER_SIZE: usize = 4;

/// Чтение одного фрейма из stdin (4 байта длина + JSON payload)
fn read_stdin_frame() -> Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    if io::stdin().read_exact(&mut len_buf).is_err() {
        return Ok(None); // EOF — браузер закрыл stdin
    }
    let length = u32::from_le_bytes(len_buf) as usize;
    if length == 0 || length > 10_000_000 {
        anyhow::bail!("Invalid message length from stdin: {}", length);
    }
    let mut payload = vec![0u8; length];
    io::stdin().read_exact(&mut payload)?;
    Ok(Some(payload))
}

/// Запись одного фрейма в stdout (4 байта длина + JSON payload)
fn write_stdout_frame(payload: &[u8]) -> Result<()> {
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(payload);
    io::stdout().write_all(&frame)?;
    io::stdout().flush()?;
    Ok(())
}

/// Чтение точного количества байт из Named Pipe (синхронный ReadFile)
fn read_exact_pipe(pipe: HANDLE, size: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; size];
    let mut offset = 0;

    while offset < size {
        let mut read = 0u32;
        let result =
            unsafe { ReadFile(pipe, Some(&mut buf[offset..]), Some(&mut read), None) };
        if let Err(e) = result {
            return Err(anyhow::anyhow!("ReadFile failed: {}", e.code()));
        }
        if read == 0 {
            if offset == 0 {
                return Err(anyhow::anyhow!("Named pipe disconnected"));
            }
            break;
        }
        offset += read as usize;
    }

    buf.truncate(offset);
    Ok(buf)
}

/// Запись всех байт в Named Pipe (синхронный WriteFile)
fn write_pipe_all(pipe: HANDLE, data: &[u8]) -> Result<()> {
    let mut offset = 0;

    while offset < data.len() {
        let mut written = 0u32;
        let result =
            unsafe { WriteFile(pipe, Some(&data[offset..]), Some(&mut written), None) };
        if let Err(e) = result {
            return Err(anyhow::anyhow!("WriteFile failed: {}", e.code()));
        }
        if written == 0 {
            return Err(anyhow::anyhow!("Named pipe write returned 0 bytes"));
        }
        offset += written as usize;
    }

    Ok(())
}

/// Дескриптор канала с автоматическим закрытием при выходе из области видимости
struct PipeGuard(HANDLE);

impl Drop for PipeGuard {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0).ok() };
    }
}

/// Основной цикл MsgHost (выполняется в `spawn_blocking` — синхронный I/O).
/// loop: stdin → decode SdaMessage → encode SdaFrame → WriteFile(pipe)
///       → ReadFile(pipe) → write stdout frame
fn run(pipe_name: &str) -> Result<()> {
    let pipe_name_wide: Vec<u16> = pipe_name.encode_utf16().collect();
    let pipe_name_ptr = PCWSTR::from_raw(pipe_name_wide.as_ptr());

    // Открываем Named Pipe на стороне клиента.
    // Без FILE_FLAG_OVERLAPPED — синхронный (блокирующий) режим,
    // согласованный с сервером SDA Core (CreateNamedPipeW без overlapped).
    let pipe = match unsafe {
        CreateFileW(
            pipe_name_ptr,
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    } {
        Ok(handle) => handle,
        Err(e) => {
            anyhow::bail!(
                "CreateFileW failed: {}. Запущен ли SDA Core (`sda serve`)?",
                e
            )
        }
    };

    if pipe == INVALID_HANDLE_VALUE {
        anyhow::bail!(
            "Не удалось подключиться к Named Pipe {}. Запущен ли SDA Core (`sda serve`)?",
            pipe_name
        );
    }

    let _pipe_guard = PipeGuard(pipe);

    eprintln!("[SDA MsgHost] Connected to Named Pipe {}", pipe_name);

    loop {
        // 1. Читаем сообщение из stdin (Native Messaging: 4 байта длина + JSON)
        let payload = match read_stdin_frame()? {
            Some(p) => p,
            None => break, // stdin закрыт браузером — завершаем работу
        };

        // 2. Декодируем SdaMessage (валидация входящего JSON)
        let _message: SdaMessage = serde_json::from_slice(&payload)
            .with_context(|| "Failed to parse SdaMessage from stdin")?;

        // 3. Кодируем в SdaFrame (4 байта длина + JSON) и пишем в Named Pipe
        let frame = SdaFrame::encode(&_message)
            .with_context(|| "Failed to encode SdaMessage to SdaFrame")?;
        write_pipe_all(pipe, &frame)?;

        // 4. Читаем ответ из Named Pipe (4 байта длина + JSON payload)
        let len_bytes = match read_exact_pipe(pipe, FRAME_HEADER_SIZE) {
            Ok(b) if b.len() == FRAME_HEADER_SIZE => b,
            _ => break, // сервер закрыл канал
        };
        let length =
            u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
        if length == 0 || length > 10_000_000 {
            anyhow::bail!("Invalid response frame size from Named Pipe: {}", length);
        }
        let response_payload = read_exact_pipe(pipe, length)?;

        // 5. Пишем ответ в stdout (4 байта длина + JSON)
        write_stdout_frame(&response_payload)?;
    }

    eprintln!("[SDA MsgHost] Loop finished");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let pipe_name = PIPE_NAME.to_owned();
    // Весь цикл ввода-вывода блокирующий, поэтому запускаем его в
    // spawn_blocking. HANDLE — Copy, поэтому проблем со временем жизни нет.
    tokio::task::spawn_blocking(move || run(&pipe_name)).await?
}
