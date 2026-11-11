// DesktopORZ — Native Rust GUI Binary
// Copyright (C) 2026 ThainanViniciusKatchan
//
// Inicia a interface gráfica nativa em Rust sem abrir janela de terminal.
#![windows_subsystem = "windows"]

use std::process::ExitCode;

fn main() -> ExitCode {
    match desktoporz::gui::run_gui() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
