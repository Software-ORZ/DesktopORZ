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

//! Compartilha os módulos do DesktopORZ entre o CLI principal
//! (`DesktopORZ.exe`) e o monitor de ocultar ícones
//! (`DesktopORZ-HideIcons.exe`, sem janela de console).

pub mod config;
pub mod hide_icons;
pub mod i18n;
pub mod layout;
pub mod process_watcher;
pub mod remote_memory;
pub mod shell_locator;
pub mod startup;
pub mod types;
pub mod wait_drive;
