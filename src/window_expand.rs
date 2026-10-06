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

//! Expansão temporária das janelas top-level para a resolução física total
//! do monitor (acionada pela flag `--include-taskbar` do `hide-icons`).
//!
//! Complementa a ocultação da barra de tarefas (`taskbar_hide.rs`): como o
//! daemon apenas altera a Work Area via `SPI_SETWORKAREA`, janelas que não
//! estão maximizadas (e algumas que ignoram `WM_SETTINGCHANGE`) continuariam
//! presas ao layout antigo. Este módulo, na transição de ocultação:
//!
//! * percorre as janelas top-level visíveis (`EnumWindows`);
//! * destrava cada uma com `ShowWindow(SW_RESTORE)` (solta o clamp do
//!   estado maximizado/`rcWork` antigo);
//! * amplia com `SetWindowPos(0, 0, SM_CXSCREEN, SM_CYSCREEN)` até a borda
//!   física total da tela, sem alterar a ordem Z (`SWP_NOZORDER`).
//!
//! Na restauração chama `ShowWindow(SW_MAXIMIZE)` nas janelas que foram
//! expandidas: com a Work Area já devolvida ao valor original, o subsistema
//! reforça a margem padrão da área útil (`rcWork`) por janela.

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetSystemMetrics, GetWindow, GetWindowLongW, GetWindowTextLengthW,
    IsIconic, IsWindow, IsWindowVisible, SetWindowPos, ShowWindow,
    GW_OWNER, GWL_EXSTYLE, SM_CXSCREEN, SM_CYSCREEN, SWP_FRAMECHANGED, SWP_NOZORDER,
    SW_MAXIMIZE, SW_RESTORE, WS_EX_TOOLWINDOW,
};

/// Gerenciador de estado da expansão de janelas.
///
/// Guarda os handles expandidos para o restauro (outras janelas abertas no
/// meio do ciclo não são tocadas). Implementa `Drop`: se o daemon encerrar
/// (retorno, erro ou unwind de panic) com janelas expandidas, elas são
/// restauradas ao estado maximizado padrão da área útil.
pub struct WindowExpander {
    /// Handles top-level que este processo expandiu — o único conjunto que
    /// o restauro tem permissão de tocar.
    expanded: Vec<HWND>,
}

impl WindowExpander {
    pub fn new() -> Self {
        Self {
            expanded: Vec::new(),
        }
    }

    /// Marca se há janelas aguardando restauro (similar ao
    /// `TaskbarHider::is_hidden`).
    pub fn is_expanded(&self) -> bool {
        !self.expanded.is_empty()
    }

    /// Expande todas as janelas top-level visíveis para a resolução física
    /// total da tela primária. Idempotente: enquanto houver janelas
    /// expandidas pendentes de restauro, não enumera de novo.
    ///
    /// Falhas individuais por janela (janela fechou entre a enumeração e a
    /// chamada, app sem permissão, etc.) são ignoradas — nunca derrubam o
    /// daemon. Erro só é propagado se a resolução não puder ser obtida.
    pub fn expand_all(&mut self) -> Result<usize, String> {
        if !self.expanded.is_empty() {
            return Ok(0);
        }
        let (width, height) = full_screen_size()?;

        let mut targets: Vec<HWND> = Vec::new();
        unsafe {
            let _ = EnumWindows(
                Some(enum_windows_proc),
                LPARAM(&mut targets as *mut Vec<HWND> as isize),
            );
        }

        for hwnd in targets {
            unsafe {
                if !IsWindow(hwnd).as_bool() {
                    continue;
                }
                // SW_RESTORE primeiro: destrava a janela do layout restrito
                // (estado maximizado preso à área útil antiga) para que o
                // SetWindowPos abaixo alcance a borda física da tela.
                let _ = ShowWindow(hwnd, SW_RESTORE);
                if SetWindowPos(hwnd, None, 0, 0, width, height, SWP_NOZORDER | SWP_FRAMECHANGED)
                    .is_ok()
                {
                    self.expanded.push(hwnd);
                }
            }
        }
        Ok(self.expanded.len())
    }

    /// Restaura as janelas expandidas: `SW_MAXIMIZE` força o subsistema a
    /// reposicionar cada janela na área útil atual (`rcWork`) — que, neste
    /// ponto do ciclo, já voltou a reservar o espaço da barra de tarefas.
    /// Handles invalidados no meio do ciclo são ignorados.
    pub fn restore_all(&mut self) -> usize {
        let mut restored = 0;
        for hwnd in std::mem::take(&mut self.expanded) {
            unsafe {
                if IsWindow(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                    restored += 1;
                }
            }
        }
        restored
    }
}

impl Drop for WindowExpander {
    /// Graceful shutdown: se o processo morrer/sair com janelas expandidas,
    /// devolve-as ao estado maximizado da área útil. Ignora erros — estamos
    /// em destruição, possivelmente durante unwind de panic.
    fn drop(&mut self) {
        let _ = self.restore_all();
    }
}

/// Resolução física total da tela primária (`SM_CXSCREEN` × `SM_CYSCREEN`).
fn full_screen_size() -> Result<(i32, i32), String> {
    let (width, height) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
    if width <= 0 || height <= 0 {
        return Err("GetSystemMetrics returned invalid screen size.".to_string());
    }
    Ok((width, height))
}

/// Callback do `EnumWindows`: filtra apenas janelas de aplicação "normais"
/// (top-level, visíveis, com título, sem dono e não-toolwindow) e acumula no
/// vetor apontado pelo `lparam`. Retorna sempre TRUE para continuar a
/// enumeração mesmo se uma janela individual falhar.
unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let targets = &mut *(lparam.0 as *mut Vec<HWND>);
    if is_expandable(hwnd) {
        targets.push(hwnd);
    }
    TRUE
}

/// Filtro de candidatas à expansão. Exclui janelas da shell (barra de
/// tarefas e desktop/WorkerW não têm título), overlays/toolwindows, janelas
/// owned (diálogos de apps) e minimizadas/miniconas (`IsIconic`).
fn is_expandable(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return false;
        }
        if GetWindowTextLengthW(hwnd) == 0 {
            return false;
        }
        // Janela owned (diálogo/modal): pertence a outra top-level; expandir
        // quebraria o layout do aplicativo dono.
        if GetWindow(hwnd, GW_OWNER).map(|o| !o.is_invalid()).unwrap_or(false) {
            return false;
        }
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return false;
        }
        true
    }
}
