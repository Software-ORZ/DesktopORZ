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

use std::thread::sleep;

use crate::i18n::t_args;
use std::time::{Duration, Instant};

use windows::core::PWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// Descrição de arquivo (FileDescription) do processo persistente do Google
/// Drive para desktop — é assim que ele aparece no Gerenciador de Tarefas.
/// O executável `GoogleDriveFS.exe` abre e se encerra em milissegundos, então
/// a detecção confiável é pela descrição do processo que permanece rodando.
/// Se uma versão do Drive usar outra descrição, basta ajustar esta constante.
pub const GOOGLE_DRIVE_DESCRIPTION: &str = "Google Drive";

/// Retorna true se um processo com o nome de exe (case-insensitive) estiver
/// rodando. Mantido como utilitário/fallback para detecção por nome.
#[allow(dead_code)]
pub fn is_process_running(process_name: &str) -> bool {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(handle) => handle,
            Err(_) => return false,
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut found = false;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case(process_name) {
                    found = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
        found
    }
}

/// Lê a FileDescription do version info de um executável.
fn file_description(exe_path: &[u16]) -> Option<String> {
    unsafe {
        let path = windows::core::PCWSTR::from_raw(exe_path.as_ptr());
        let mut dw_handle = 0u32;
        let size = GetFileVersionInfoSizeW(path, Some(&mut dw_handle));
        if size == 0 {
            return None;
        }

        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(path, dw_handle, size, data.as_mut_ptr() as *mut _).ok()?;

        // Descobre lang/codepage declarados no bloco de tradução.
        let mut buf: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let translations: Vec<(u16, u16)> = if VerQueryValueW(
            data.as_ptr() as *const _,
            windows::core::w!("\\VarFileInfo\\Translation"),
            &mut buf,
            &mut len,
        )
        .as_bool()
            && !buf.is_null()
            && len >= 4
        {
            let count = (len as usize) / 4;
            let pairs = std::slice::from_raw_parts(buf as *const u32, count);
            pairs
                .iter()
                .map(|v| ((v & 0xFFFF) as u16, (v >> 16) as u16))
                .collect()
        } else {
            // Fallback comum: inglês (EUA) com codepage Unicode/ANSI.
            vec![(0x0409, 0x04B0), (0x0409, 0x04E4)]
        };

        for (lang, codepage) in translations {
            let sub_block = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription");
            let sub_block_wide: Vec<u16> =
                sub_block.encode_utf16().chain(std::iter::once(0)).collect();
            let mut value: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut value_len = 0u32;
            if VerQueryValueW(
                data.as_ptr() as *const _,
                windows::core::PCWSTR::from_raw(sub_block_wide.as_ptr()),
                &mut value,
                &mut value_len,
            )
            .as_bool()
                && !value.is_null()
                && value_len > 0
            {
                let chars = std::slice::from_raw_parts(
                    value as *const u16,
                    (value_len as usize).saturating_sub(1),
                );
                return Some(String::from_utf16_lossy(chars));
            }
        }
        None
    }
}

/// Retorna true se algum processo em execução tiver a FileDescription
/// informada (case-insensitive).
pub fn is_process_running_with_description(description: &str) -> bool {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(handle) => handle,
            Err(_) => return false,
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut found = false;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32ProcessID != 0 {
                    if let Ok(process) = OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION,
                        false,
                        entry.th32ProcessID,
                    ) {
                        let mut buffer = [0u16; 1024];
                        let mut size = buffer.len() as u32;
                        if QueryFullProcessImageNameW(
                            process,
                            PROCESS_NAME_WIN32,
                            PWSTR::from_raw(buffer.as_mut_ptr()),
                            &mut size,
                        )
                        .is_ok()
                        {
                            // Termina o buffer em nulo para as funções de version info.
                            let end = (size as usize).min(buffer.len() - 1);
                            buffer[end] = 0;
                            if let Some(desc) = file_description(&buffer[..=end]) {
                                if desc.trim().eq_ignore_ascii_case(description) {
                                    found = true;
                                }
                            }
                        }
                        let _ = CloseHandle(process);
                        if found {
                            break;
                        }
                    }
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
        found
    }
}

/// Espera o processo do Google Drive aparecer (identificado pela
/// FileDescription) e depois aguarda um tempo de estabilização para que ele
/// termine de criar/modificar os ícones da área de trabalho.
/// Retorna quanto tempo esperou até o processo ser detectado.
pub fn wait_for_process(
    description: &str,
    timeout: Option<Duration>,
    settle: Duration,
) -> Result<Duration, String> {
    let start = Instant::now();
    let poll = Duration::from_secs(1);

    loop {
        if is_process_running_with_description(description) {
            let waited = start.elapsed();
            if !settle.is_zero() {
                sleep(settle);
            }
            return Ok(waited);
        }
        if let Some(limit) = timeout {
            if start.elapsed() >= limit {
                return Err(t_args(
                    "process_watcher.timeout",
                    &[
                        ("process", description),
                        ("seconds", &limit.as_secs().to_string()),
                    ],
                ));
            }
        }
        sleep(poll);
    }
}
