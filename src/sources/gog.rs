use crate::model::{Game, Installation, Launcher};
use std::{fs, path::PathBuf};

pub fn discover() -> Vec<Game> {
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        if let Some(program_files) = std::env::var_os("ProgramFiles(x86)") {
            roots.push(PathBuf::from(program_files).join("GOG Galaxy/Games"));
        }
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            roots.push(PathBuf::from(program_files).join("GOG Galaxy/Games"));
        }
    }
    if let Some(home) = dirs::home_dir() {
        roots.extend([home.join("GOG Games"), home.join("Games/GOG")]);
    }
    let mut result = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten().filter(|e| e.path().is_dir()) {
            let install = entry.path();
            let Some(title) = install
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
            else {
                continue;
            };
            result.push(Game::from_installation(
                title,
                "GOG",
                Installation {
                    path: install,
                    launcher: Launcher::Gog,
                    launch_profile: None,
                    available: true,
                },
            ));
        }
    }
    result
}
