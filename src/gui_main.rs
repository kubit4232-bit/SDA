//! Точка входа для SDA GUI — нативный Win32-интерфейс в стиле IDM.
//! Запуск: cargo run --bin sda-gui --features gui --release

#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    sda_core::native_gui::run()
}
