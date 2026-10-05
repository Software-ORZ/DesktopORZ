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

use std::collections::HashMap;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windows::core::Result;
use windows::core::PWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Controls::{
    LVIF_TEXT, LVITEMW, LVM_GETITEMCOUNT, LVM_GETITEMPOSITION, LVM_GETITEMTEXTW,
};

/// LVM_SETITEMPOSITION32 — define a posição via POINT* de 32 bits em vez de
/// coordenadas de 16 bits empacotadas no LPARAM. Necessário para coordenadas
/// negativas (multi-monitor) ou acima de 32767; com LVM_SETITEMPOSITION o
/// truncamento corrompe a posição e o ícone fica "solto".
const LVM_SETITEMPOSITION32: u32 = 0x1000 + 49;
use windows::Win32::UI::WindowsAndMessaging::SendMessageW;

use crate::remote_memory::{ProcessHandle, RemoteBuffer};
use crate::shell_locator;
use crate::types::{DesktopProfile, IconEntry, Resolution};

const TEXT_BUF_CHARS: usize = 512;

pub fn save_layout(profile_name: &str, resolution: Resolution) -> Result<DesktopProfile> {
    let listview = shell_locator::find_desktop_listview()?;
    let process = shell_locator::open_explorer_process(listview)?;

    let count = unsafe { SendMessageW(listview, LVM_GETITEMCOUNT, None, LPARAM(0)).0 } as usize;

    let point_buf = RemoteBuffer::new(&process, std::mem::size_of::<POINT>())?;
    let item_buf = RemoteBuffer::new(&process, std::mem::size_of::<LVITEMW>())?;
    let text_buf = RemoteBuffer::new(&process, TEXT_BUF_CHARS * 2)?;

    let mut icons = Vec::with_capacity(count);
    for index in 0..count {
        let (name, point) = read_item(listview, &process, &point_buf, &item_buf, &text_buf, index)?;
        icons.push(IconEntry {
            name,
            x: point.x,
            y: point.y,
        });
    }

    let timestamp_utc = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();

    Ok(DesktopProfile {
        profile_name: profile_name.to_string(),
        resolution,
        timestamp_utc,
        icons,
        resolutions: HashMap::new(),
    })
}

/// Quantas vezes reaplicamos o layout se o Explorer bagunçar as posições
/// logo após a restauração (acontece no logon, enquanto o desktop ainda
/// está inicializando e o Explorer rearranja os ícones por conta própria).
const RESTORE_ATTEMPTS: usize = 10;
const RETRY_DELAY: Duration = Duration::from_millis(750);
const DESKTOP_WAIT_TIMEOUT: Duration = Duration::from_secs(60);
const DESKTOP_POLL_INTERVAL: Duration = Duration::from_millis(500);

pub fn restore_layout(profile: &DesktopProfile) -> Result<usize> {
    let listview = wait_for_desktop()?;
    let process = shell_locator::open_explorer_process(listview)?;

    let count = unsafe { SendMessageW(listview, LVM_GETITEMCOUNT, None, LPARAM(0)).0 } as usize;

    let point_buf = RemoteBuffer::new(&process, std::mem::size_of::<POINT>())?;
    let item_buf = RemoteBuffer::new(&process, std::mem::size_of::<LVITEMW>())?;
    let text_buf = RemoteBuffer::new(&process, TEXT_BUF_CHARS * 2)?;

    let mut name_to_index: HashMap<String, usize> = HashMap::with_capacity(count);
    for index in 0..count {
        let (name, _) = read_item(listview, &process, &point_buf, &item_buf, &text_buf, index)?;
        name_to_index.insert(name, index);
    }

    let current = shell_locator::current_resolution();
    let res_key = format!("{}x{}", current.width, current.height);
    let icons = profile.resolutions.get(&res_key).unwrap_or(&profile.icons);

    // Aplica e verifica. No logon, o Explorer pode mover os ícones depois da
    // primeira aplicação; relemos as posições e reaplicamos os divergentes.
    let mut moved = 0usize;
    for attempt in 0..RESTORE_ATTEMPTS {
        if attempt > 0 {
            sleep(RETRY_DELAY);
        }

        let mut divergent = 0usize;
        for icon in icons {
            if let Some(&index) = name_to_index.get(&icon.name) {
                if attempt > 0 {
                    let (_, current_pos) =
                        read_item(listview, &process, &point_buf, &item_buf, &text_buf, index)?;
                    if current_pos.x == icon.x && current_pos.y == icon.y {
                        continue;
                    }
                }

                let point = POINT {
                    x: icon.x,
                    y: icon.y,
                };
                point_buf.write(&point)?;
                unsafe {
                    SendMessageW(
                        listview,
                        LVM_SETITEMPOSITION32,
                        WPARAM(index),
                        LPARAM(point_buf.as_ptr() as isize),
                    );
                }
                divergent += 1;
            }
        }

        shell_locator::refresh(listview);
        moved = moved.max(divergent);

        if divergent == 0 {
            break;
        }
    }

    Ok(moved)
}

/// Aguarda o desktop (SysListView32) existir e já ter ícones. No logon
/// automático o Explorer pode ainda não ter criado a janela do desktop.
fn wait_for_desktop() -> Result<HWND> {
    let start = Instant::now();
    loop {
        if let Ok(listview) = shell_locator::find_desktop_listview() {
            let count =
                unsafe { SendMessageW(listview, LVM_GETITEMCOUNT, None, LPARAM(0)).0 } as usize;
            if count > 0 {
                return Ok(listview);
            }
        }
        if start.elapsed() >= DESKTOP_WAIT_TIMEOUT {
            // Deixa o erro original do find_desktop_listview subir.
            return shell_locator::find_desktop_listview();
        }
        sleep(DESKTOP_POLL_INTERVAL);
    }
}

fn read_item(
    listview: HWND,
    _process: &ProcessHandle,
    point_buf: &RemoteBuffer,
    item_buf: &RemoteBuffer,
    text_buf: &RemoteBuffer,
    index: usize,
) -> Result<(String, POINT)> {
    unsafe {
        SendMessageW(
            listview,
            LVM_GETITEMPOSITION,
            WPARAM(index),
            LPARAM(point_buf.as_ptr() as isize),
        );
    }
    let point: POINT = point_buf.read()?;

    let item = LVITEMW {
        mask: LVIF_TEXT,
        iItem: index as i32,
        pszText: PWSTR(text_buf.as_ptr() as *mut u16),
        cchTextMax: TEXT_BUF_CHARS as i32,
        ..Default::default()
    };
    item_buf.write(&item)?;

    unsafe {
        SendMessageW(
            listview,
            LVM_GETITEMTEXTW,
            WPARAM(index),
            LPARAM(item_buf.as_ptr() as isize),
        );
    }
    let name = text_buf.read_wide_string()?;

    Ok((name, point))
}
