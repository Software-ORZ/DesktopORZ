// DesktopORZ — Save and restore Windows desktop icon layouts
// Copyright (C) 2026 ThainanViniciusKatchan
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

// Sem janela: este binário roda em segundo plano (iniciado no login pela
// chave Run ou pelo comando `hide-icons on`) e nunca abre um console.
#![windows_subsystem = "windows"]

use std::process::ExitCode;

fn main() -> ExitCode {
    // As flags --kb/--keyboard, --mou/--mouse, --include-cursor e
    // --include-taskbar chegam na linha de comando (da entrada Run ou do
    // spawn do CLI); sem flags de fonte, o fallback é ambos.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let source = desktoporz::hide_icons::InputSource::from_args(&args);
    let include_cursor = desktoporz::hide_icons::include_cursor_arg(&args);
    let include_taskbar = desktoporz::hide_icons::include_taskbar_arg(&args);
    match desktoporz::hide_icons::run(source, include_cursor, include_taskbar) {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
