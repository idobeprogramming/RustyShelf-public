use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Launcher {
    Steam,
    Epic,
    Heroic,
    Lutris,
    Gog,
    Manual,
    Unknown,
}

impl Launcher {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Steam => "Steam",
            Self::Epic => "Epic Games",
            Self::Heroic => "Heroic",
            Self::Lutris => "Lutris",
            Self::Gog => "GOG",
            Self::Manual => "Ajout manuel",
            Self::Unknown => "À confirmer",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchProfile {
    pub command: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub launcher_command: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Installation {
    pub path: PathBuf,
    pub launcher: Launcher,
    pub launch_profile: Option<LaunchProfile>,
    pub available: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub category: String,
    pub favorite: bool,
    pub confirmed: bool,
    pub cover_path: Option<PathBuf>,
    pub installations: Vec<Installation>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Emulator {
    pub id: String,
    pub name: String,
    pub executable: PathBuf,
}

impl Emulator {
    pub fn new(name: impl Into<String>, executable: PathBuf) -> Self {
        let name = name.into();
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        executable.hash(&mut hasher);
        let id = format!("emulator-{:016x}", hasher.finish());
        Self {
            id,
            name,
            executable,
        }
    }
}

impl Game {
    pub fn from_installation(
        title: impl Into<String>,
        category: impl Into<String>,
        installation: Installation,
    ) -> Self {
        let title = title.into();
        let id = stable_id(&title, &installation.path);
        Self {
            id,
            title,
            category: category.into(),
            favorite: false,
            confirmed: installation.launcher != Launcher::Unknown,
            cover_path: None,
            installations: vec![installation],
        }
    }
}

pub fn stable_id(title: &str, _path: &std::path::Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    title.to_lowercase().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_installation_has_stable_id() {
        assert_eq!(
            stable_id("Celeste", std::path::Path::new("/games/celeste")),
            stable_id("celeste", std::path::Path::new("/games/celeste"))
        );
    }
}
