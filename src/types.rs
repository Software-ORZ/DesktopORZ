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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconEntry {
    pub name: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopProfile {
    pub profile_name: String,
    pub resolution: Resolution,
    pub timestamp_utc: u64,
    pub icons: Vec<IconEntry>,
    /// Layouts específicos por resolução, indexados por "LARGURAxALTURA".
    #[serde(default)]
    pub resolutions: std::collections::HashMap<String, Vec<IconEntry>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_resolution_equality() {
        let r1 = Resolution { width: 1920, height: 1080 };
        let r2 = Resolution { width: 1920, height: 1080 };
        let r3 = Resolution { width: 2560, height: 1440 };
        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }

    #[test]
    fn test_desktop_profile_serialization() {
        let mut resolutions = HashMap::new();
        resolutions.insert(
            "1920x1080".to_string(),
            vec![IconEntry {
                name: "Este Computador".to_string(),
                x: 100,
                y: 200,
            }],
        );

        let profile = DesktopProfile {
            profile_name: "padrao".to_string(),
            resolution: Resolution { width: 1920, height: 1080 },
            timestamp_utc: 1700000000,
            icons: vec![
                IconEntry {
                    name: "Lixeira".to_string(),
                    x: 10,
                    y: 20,
                },
                IconEntry {
                    name: "MultiMonitor".to_string(),
                    x: -1920,
                    y: 100,
                },
            ],
            resolutions,
        };

        let json = serde_json::to_string(&profile).expect("falha ao serializar");
        let deserialized: DesktopProfile = serde_json::from_str(&json).expect("falha ao desserializar");

        assert_eq!(deserialized.profile_name, "padrao");
        assert_eq!(deserialized.resolution.width, 1920);
        assert_eq!(deserialized.resolution.height, 1080);
        assert_eq!(deserialized.icons.len(), 2);
        assert_eq!(deserialized.icons[1].x, -1920);
        assert_eq!(deserialized.resolutions.len(), 1);
        assert_eq!(deserialized.resolutions.get("1920x1080").unwrap()[0].name, "Este Computador");
    }

    #[test]
    fn test_desktop_profile_default_resolutions() {
        let json = r#"{
            "profile_name": "simples",
            "resolution": {"width": 1280, "height": 720},
            "timestamp_utc": 12345,
            "icons": []
        }"#;

        let profile: DesktopProfile = serde_json::from_str(json).expect("falha ao desserializar json sem resolutions");
        assert_eq!(profile.profile_name, "simples");
        assert!(profile.resolutions.is_empty());
    }
}
