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

use windows::core::w;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
};
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetDesktopWindow, GetForegroundWindow, GetShellWindow, GetWindowRect,
    IsIconic, IsWindow, IsWindowVisible,
};

const STEAM_KEY: windows::core::PCWSTR = w!("Software\\Valve\\Steam");
const VALUE_RUNNING_APP_ID: windows::core::PCWSTR = w!("RunningAppID");

fn is_shell_busy_or_game() -> bool {
    unsafe {
        if let Ok(state) = SHQueryUserNotificationState() {
            return matches!(
                state,
                QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_BUSY | QUNS_PRESENTATION_MODE
            );
        }
    }
    false
}

fn is_steam_game_running() -> bool {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, STEAM_KEY, 0, KEY_QUERY_VALUE, &mut key).is_ok() {
            let mut app_id: u32 = 0;
            let mut size = std::mem::size_of::<u32>() as u32;
            let status = RegQueryValueExW(
                key,
                VALUE_RUNNING_APP_ID,
                None,
                None,
                Some(&mut app_id as *mut u32 as *mut u8),
                Some(&mut size),
            );
            let _ = RegCloseKey(key);
            if status.is_ok() && app_id != 0 {
                return true;
            }
        }
    }
    false
}

fn is_shell_or_desktop_window(hwnd: HWND) -> bool {
    unsafe {
        if hwnd == GetDesktopWindow() || hwnd == GetShellWindow() {
            return true;
        }
        let mut class_name = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class_name) as usize;
        if len > 0 {
            let class_str = String::from_utf16_lossy(&class_name[..len]);
            if matches!(
                class_str.as_str(),
                "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
            ) {
                return true;
            }
        }
    }
    false
}

pub fn is_window_fullscreen(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindow(hwnd).as_bool() || !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return false;
        }
        if is_shell_or_desktop_window(hwnd) {
            return false;
        }
        let mut window_rect = RECT::default();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return false;
        }
        let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if hmon.is_invalid() {
            return false;
        }
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(hmon, &mut mi).as_bool() {
            return false;
        }
        window_rect.left <= mi.rcMonitor.left
            && window_rect.top <= mi.rcMonitor.top
            && window_rect.right >= mi.rcMonitor.right
            && window_rect.bottom >= mi.rcMonitor.bottom
    }
}

fn is_foreground_window_fullscreen() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_invalid() {
            return false;
        }
        is_window_fullscreen(fg)
    }
}

pub fn is_fullscreen_or_game_active() -> bool {
    if is_shell_busy_or_game() {
        return true;
    }
    if is_steam_game_running() {
        return true;
    }
    if is_foreground_window_fullscreen() {
        return true;
    }
    false
}
