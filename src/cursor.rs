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

//! Ocultação do cursor global do sistema via `SetSystemCursor` (User32).
//!
//! Estratégia: cria-se um cursor transparente 1bpp (`CreateCursor` com
//! máscara AND em 1s e XOR em 0s) e ele substitui o cursor padrão
//! (`OCR_NORMAL`). O handle passado ao `SetSystemCursor` é **consumido**
//! pelo sistema operacional, portanto nunca é destruído manualmente com
//! `DestroyCursor` (apenas em caso de falha da chamada).
//!
//! O restauro é limpo e stateless: `SystemParametersInfoW(SPI_SETCURSORS)`
//! recarrega todos os cursores a partir do registro/perfil atual.

use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{BOOL, FALSE, HINSTANCE};
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateCursor, DestroyCursor, GetSystemMetrics, SetSystemCursor, SystemParametersInfoW,
    OCR_NORMAL, SM_CXCURSOR, SM_CYCURSOR, SPI_SETCURSORS,
};
use windows::Win32::UI::WindowsAndMessaging::HCURSOR;

/// Marca se o cursor foi ocultado por este processo. Atômico porque é lido
/// pelo callback do hook de input e pelo tratador de console (threads
/// distintas do loop principal).
static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

/// Cria um cursor totalmente transparente do tamanho de cursor do sistema.
fn create_blank_cursor() -> Result<HCURSOR, String> {
    let width = unsafe { GetSystemMetrics(SM_CXCURSOR) };
    let height = unsafe { GetSystemMetrics(SM_CYCURSOR) };
    if width <= 0 || height <= 0 {
        return Err("Could not query the system cursor size.".to_string());
    }
    // Bitmap monocromático: cada linha alinhada em 16 bits (2 bytes).
    let row_bytes = ((width as usize + 15) / 16) * 2;
    // AND = 1s → o pixel existente é preservado; XOR = 0s → sem desenho.
    let and_mask = vec![0xFFu8; row_bytes * height as usize];
    let xor_mask = vec![0x00u8; row_bytes * height as usize];
    let cursor = unsafe {
        CreateCursor(
            HINSTANCE::default(),
            0,
            0,
            width,
            height,
            and_mask.as_ptr() as *const _,
            xor_mask.as_ptr() as *const _,
        )
    }
    .map_err(|e| format!("CreateCursor failed: {e}"))?;
    Ok(cursor)
}

/// Substitui o cursor padrão do sistema (`OCR_NORMAL`) por um cursor
/// transparente. Idempotente: chamadas repetidas enquanto oculto são no-op.
pub fn hide_system_cursor() -> Result<(), String> {
    if CURSOR_HIDDEN.load(Ordering::Relaxed) {
        return Ok(());
    }
    let blank = create_blank_cursor()?;
    if let Err(e) = unsafe { SetSystemCursor(blank, OCR_NORMAL) } {
        // Falhou: o handle NÃO foi consumido; destruir para não vazar GDI.
        unsafe {
            let _ = DestroyCursor(blank);
        }
        return Err(format!("SetSystemCursor failed: {e}"));
    }
    // Sucesso: o sistema passou a possuir `blank`; não chamar DestroyCursor.
    CURSOR_HIDDEN.store(true, Ordering::Relaxed);
    Ok(())
}

/// Restaura os cursores padrão do Windows recarregando-os do registro
/// (`SPI_SETCURSORS`). Idempotente e seguro de chamar em qualquer contexto.
pub fn restore_system_cursor() {
    if CURSOR_HIDDEN.swap(false, Ordering::Relaxed) {
        unsafe {
            let _ = SystemParametersInfoW(SPI_SETCURSORS, 0, Some(null_mut()), Default::default());
        }
    }
}

/// Tratador de eventos de console (Ctrl+C, fechar janela, logoff): restaura
/// o cursor e retorna FALSE para seguir com o encerramento padrão. No daemon
/// (subsistema Windows, sem console) o registro falha silenciosamente e a
/// proteção fica a cargo do `Drop`.
unsafe extern "system" fn console_ctrl_handler(_ctrl_type: u32) -> BOOL {
    restore_system_cursor();
    FALSE
}

/// Guarda RAII: restaura o cursor original ao sair de escopo — inclusive
/// durante o unwind de um panic — para o usuário nunca ficar sem ponteiro.
/// Também instala o tratador de console (Ctrl+C) quando há um console.
pub struct SystemCursorGuard;

impl SystemCursorGuard {
    pub fn install() -> Self {
        unsafe {
            let _ = SetConsoleCtrlHandler(Some(console_ctrl_handler), true);
        }
        Self
    }
}

impl Drop for SystemCursorGuard {
    fn drop(&mut self) {
        restore_system_cursor();
    }
}
