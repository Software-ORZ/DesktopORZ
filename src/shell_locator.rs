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

use windows::core::{w, Error, Result, PCWSTR};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetWindowThreadProcessId,
};

use crate::remote_memory::ProcessHandle;

pub fn find_desktop_listview() -> Result<HWND> {
    unsafe {
        let progman = FindWindowW(w!("Progman"), PCWSTR::null())?;
        if let Some(listview) = find_listview_under(progman) {
            return Ok(listview);
        }

        let mut worker = HWND::default();
        loop {
            worker = FindWindowExW(HWND::default(), worker, w!("WorkerW"), PCWSTR::null())?;
            if worker.is_invalid() {
                break;
            }
            if let Some(listview) = find_listview_under(worker) {
                return Ok(listview);
            }
        }

        Err(Error::from_win32())
    }
}

fn find_listview_under(parent: HWND) -> Option<HWND> {
    unsafe {
        let def_view = FindWindowExW(
            parent,
            HWND::default(),
            w!("SHELLDLL_DefView"),
            PCWSTR::null(),
        )
        .ok()?;
        if def_view.is_invalid() {
            return None;
        }
        let listview = FindWindowExW(
            def_view,
            HWND::default(),
            w!("SysListView32"),
            PCWSTR::null(),
        )
        .ok()?;
        if listview.is_invalid() {
            return None;
        }
        Some(listview)
    }
}

pub fn open_explorer_process(listview: HWND) -> Result<ProcessHandle> {
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
    };

    unsafe {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(listview, Some(&mut pid));
        if pid == 0 {
            return Err(Error::from_win32());
        }
        let handle = OpenProcess(
            PROCESS_VM_OPERATION | PROCESS_VM_READ | PROCESS_VM_WRITE,
            false,
            pid,
        )?;
        Ok(ProcessHandle::new(handle))
    }
}

pub fn current_resolution() -> crate::types::Resolution {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        crate::types::Resolution {
            width: GetSystemMetrics(SM_CXSCREEN) as u32,
            height: GetSystemMetrics(SM_CYSCREEN) as u32,
        }
    }
}

/// Atualiza a área de trabalho (equivalente ao F5), sem bloquear.
///
/// Envia a tecla F5 para o SysListView32 da área de trabalho e notifica o
/// shell via SHChangeNotify (fallback que funciona mesmo se a janela não for
/// encontrada).
pub fn refresh_desktop() -> Result<()> {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_F5;
    use windows::Win32::UI::Shell::{
        SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_FLUSH, SHCNF_IDLIST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_KEYDOWN, WM_KEYUP};

    if let Ok(listview) = find_desktop_listview() {
        unsafe {
            let wparam = WPARAM(VK_F5.0 as usize);
            let _ = PostMessageW(listview, WM_KEYDOWN, wparam, LPARAM(0));
            let _ = PostMessageW(listview, WM_KEYUP, wparam, LPARAM(0));
        }
    }

    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST | SHCNF_FLUSH,
            None,
            None,
        );
    }
    Ok(())
}

pub fn refresh(listview: HWND) {
    use windows::Win32::Graphics::Gdi::HRGN;
    use windows::Win32::Graphics::Gdi::{
        InvalidateRect, RedrawWindow, RDW_ERASE, RDW_INVALIDATE, RDW_UPDATENOW,
    };
    unsafe {
        let _ = InvalidateRect(listview, None::<*const RECT>, true);
        let _ = RedrawWindow(
            listview,
            None,
            HRGN::default(),
            RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW,
        );
    }
}
