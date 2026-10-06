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

//! Ocultação temporária da barra de tarefas do Windows (Taskbar Hide on AFK).
//!
//! Estratégia: **não** usamos a alteração nativa de auto-hide da shell
//! (`SHAppBarMessage(ABM_SETSTATE, ABS_AUTOHIDE)`), que causa micro-travamentos
//! no Explorer e o efeito "sanfona" das janelas. Em vez disso, a barra é
//! ocultada diretamente com `ShowWindow(SW_HIDE)` e a Área Útil de Trabalho
//! (Work Area) é expandida/encolhida via `SystemParametersInfoW(SPI_SETWORKAREA)`.
//!
//! O sequenciamento dos passos é rigoroso para eliminar flickering:
//!
//! * **Ocultar** — primeiro `SW_HIDE` na barra, **depois** expande a Work
//!   Area e só então notifica. Assim as janelas maximizadas expandem quando a
//!   barra já sumiu (sem barra visível sobre janela redimensionada).
//! * **Restaurar** — primeiro encolhe a Work Area para o valor original,
//!   **depois** `SW_SHOW` na barra com redraw forçado, e por fim notifica.
//!   Assim as janelas encolhem quando a barra já reapareceu (sem buraco preto).

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    RedrawWindow, RDW_ALLCHILDREN, RDW_INVALIDATE, RDW_UPDATENOW, HRGN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetSystemMetrics, IsWindow, ShowWindow, HWND_BROADCAST,
    SystemParametersInfoW, SendMessageTimeoutW, SMTO_ABORTIFHUNG, SMTO_NORMAL,
    SM_CXSCREEN, SM_CYSCREEN, SPIF_SENDCHANGE, SPI_GETWORKAREA, SPI_SETWORKAREA,
    SW_HIDE, SW_SHOW, WM_SETTINGCHANGE,
};

/// Timeout do broadcast `WM_SETTINGCHANGE` (ms): curto o suficiente para um
/// aplicativo "travado" não congelar a thread do daemon; a flag
/// `SMTO_ABORTIFHUNG` dispensa janelas que já não respondem.
const BROADCAST_TIMEOUT_MS: u32 = 100;

/// Gerenciador de estado da ocultação da barra de tarefas.
///
/// Mantém a Work Area original e o estado atual para evitar chamadas
/// redundantes à API. Implementa `Drop`: se o daemon encerrar (retorno,
/// erro ou unwind de panic) com a barra oculta, ela é restaurada.
pub struct TaskbarHider {
    /// Área Útil de Trabalho original, capturada **apenas na primeira**
    /// transição para oculto — nunca sobrescrita pela área expandida.
    original_work_area: RECT,
    /// Marca se `original_work_area` já foi preenchida (a barra pode nunca
    /// ter sido ocultada; nesse caso não há nada a restaurar).
    work_area_saved: bool,
    /// Estado atual: evita `ShowWindow`/`SPI_SETWORKAREA` repetidos a cada
    /// ciclo de varredura do daemon.
    is_hidden: bool,
}

impl TaskbarHider {
    pub fn new() -> Self {
        Self {
            original_work_area: RECT::default(),
            work_area_saved: false,
            is_hidden: false,
        }
    }

    pub fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    /// Alterna a visibilidade apenas na transição de estado (idempotente).
    pub fn set_hidden(&mut self, hidden: bool) -> Result<(), String> {
        if hidden {
            self.hide()
        } else {
            self.show()
        }
    }

    /// Sequência rigorosa de ocultação:
    /// 1. `SW_HIDE` na barra principal e secundárias.
    /// 2. Expande a Work Area para a tela primária inteira.
    /// 3. Broadcast leve (`WM_SETTINGCHANGE`) com timeout.
    pub fn hide(&mut self) -> Result<(), String> {
        if self.is_hidden {
            return Ok(());
        }

        // Salva a Work Area original uma única vez, antes de qualquer
        // modificação. Se já estiver salva (ciclos hide→show→hide),
        // reutiliza o valor original — nunca o expandido.
        if !self.work_area_saved {
            self.original_work_area = query_work_area()?;
            self.work_area_saved = true;
        }

        // 1) Some com a barra PRIMEIRO: qualquer janela que reagir à
        //    expansão da Work Area não sofrerá redraw sob a barra visível.
        for taskbar in find_taskbar_windows() {
            hide_window(taskbar);
        }

        // 2) Expande a Work Area para a tela primária inteira. Com
        //    SPIF_SENDCHANGE o próprio sistema já envia WM_SETTINGCHANGE.
        let full = full_screen_rect()?;
        apply_work_area(full)?;

        // 3) Broadcast com timeout: garante que aplicativos que filtram por
        //    wParam == SPI_SETWORKAREA recalculsem limites, sem arriscar
        //    travar a thread se algum app estiver pendurado.
        broadcast_work_area_changed();

        self.is_hidden = true;
        Ok(())
    }

    /// Sequência rigorosa de restauração:
    /// 1. Encolhe a Work Area de volta ao valor original.
    /// 2. `SW_SHOW` imediato na barra + redraw forçado.
    /// 3. Broadcast leve para as janelas recalcularem seus limites.
    pub fn show(&mut self) -> Result<(), String> {
        if !self.is_hidden {
            return Ok(());
        }

        // 1) Restaura a Work Area ANTES de reexibir a barra: as janelas
        //    maximizadas recebem a notificação e encolhem para deixar o
        //    espaço da barra — que já estará visível, sem "buraco preto".
        if self.work_area_saved {
            apply_work_area(self.original_work_area)?;
        }

        // 2) Reexibe imediatamente e força repaint síncrono da barra e dos
        //    filhos (RDW_UPDATENOW) para não ficarem artefatos de janela.
        for taskbar in find_taskbar_windows() {
            show_window(taskbar);
        }

        // 3) Broadcast com timeout curto: janelas terminam de recalcular
        //    seus limites sem risco de travar a thread do daemon.
        broadcast_work_area_changed();

        self.is_hidden = false;
        Ok(())
    }
}

impl Drop for TaskbarHider {
    /// Graceful shutdown: se o processo morrer/sair com a barra oculta,
    /// restaura-a e devolve a Work Area original. Ignora erros — estamos em
    /// destruição, possivelmente durante unwind de panic.
    fn drop(&mut self) {
        let _ = self.show();
    }
}

/// Lê a Work Area atual (que, antes da primeira ocultação, é a original
/// configurada pelo usuário/shell).
fn query_work_area() -> Result<RECT, String> {
    unsafe {
        let mut rect = RECT::default();
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut _),
            Default::default(),
        )
        .map_err(|e| format!("SPI_GETWORKAREA failed: {e}"))?;
        Ok(rect)
    }
}

/// Retângulo da tela primária inteira (Work Area "maximizada" para quando a
/// barra está oculta).
fn full_screen_rect() -> Result<RECT, String> {
    unsafe {
        let cx = GetSystemMetrics(SM_CXSCREEN);
        let cy = GetSystemMetrics(SM_CYSCREEN);
        if cx <= 0 || cy <= 0 {
            return Err("GetSystemMetrics returned invalid screen size.".to_string());
        }
        Ok(RECT {
            left: 0,
            top: 0,
            right: cx,
            bottom: cy,
        })
    }
}

/// Aplica a Work Area via `SPI_SETWORKAREA` com `SPIF_SENDCHANGE` (o sistema
/// dispara `WM_SETTINGCHANGE` para as janelas top-level).
fn apply_work_area(mut rect: RECT) -> Result<(), String> {
    unsafe {
        SystemParametersInfoW(
            SPI_SETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut _),
            SPIF_SENDCHANGE,
        )
        .map_err(|e| format!("SPI_SETWORKAREA failed: {e}"))
    }
}

/// Broadcast leve do `WM_SETTINGCHANGE` com `wParam = SPI_SETWORKAREA`.
///
/// `SendMessageTimeoutW` + `SMTO_ABORTIFHUNG | SMTO_NORMAL`: retorna ao fim
/// de 100ms mesmo que algum aplicativo não processe a mensagem, impedindo
/// engasgos na thread do daemon. Falhas individuais são ignoradas.
fn broadcast_work_area_changed() {
    unsafe {
        let mut result: usize = 0;
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(SPI_SETWORKAREA.0 as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG | SMTO_NORMAL,
            BROADCAST_TIMEOUT_MS,
            Some(&mut result as *mut usize),
        );
    }
}

/// Enumera defensivamente as janelas da barra de tarefas: a principal
/// (`Shell_TrayWnd`) e zero ou mais secundárias (`Shell_SecondaryTrayWnd`,
/// uma por monitor adicional). Handles nulos ou já invalidados (Explorer
/// reiniciado, monitor desligado) são descartados via `IsWindow`.
fn find_taskbar_windows() -> Vec<HWND> {
    let mut taskbars = Vec::new();
    unsafe {
        if let Ok(main) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) {
            if !main.is_invalid() && IsWindow(main).as_bool() {
                taskbars.push(main);
            }
        }
        // As secundárias são janelas top-level irmãs: FindWindowExW com pai
        // nulo e "child after" encadeado percorre todas as instâncias.
        let mut prev = HWND::default();
        loop {
            let secondary =
                FindWindowExW(HWND::default(), prev, w!("Shell_SecondaryTrayWnd"), PCWSTR::null())
                    .unwrap_or_default();
            if secondary.is_invalid() {
                break;
            }
            if IsWindow(secondary).as_bool() {
                taskbars.push(secondary);
            }
            prev = secondary;
        }
    }
    taskbars
}

/// Oculta uma janela da barra. Revalida o handle (pode estar defasado entre
/// a enumeração e a chamada).
fn hide_window(hwnd: HWND) {
    unsafe {
        if IsWindow(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

/// Reexibe uma janela da barra e força redraw síncrono (`RDW_INVALIDATE |
/// RDW_UPDATENOW | RDW_ALLCHILDREN`): evita o flicker causado quando o
/// repaint da barra fica enfileirado atrás do resize das janelas.
fn show_window(hwnd: HWND) {
    unsafe {
        if IsWindow(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = RedrawWindow(
                hwnd,
                None,
                HRGN::default(),
                RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
            );
        }
    }
}
