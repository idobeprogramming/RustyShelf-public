use crate::model::{Game, Installation, LaunchProfile, Launcher};
use std::{fs, path::PathBuf};
use walkdir::WalkDir;

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
            let executable =
                WalkDir::new(&install)
                    .max_depth(3)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(Result::ok)
                    .find(|entry| {
                        if !entry.file_type().is_file() {
                            return false;
                        }
                        if !entry.path().file_stem().is_some_and(|stem| {
                            crate::scan::looks_like_game(&stem.to_string_lossy())
                        }) {
                            return false;
                        }
                        let extension = entry
                            .path()
                            .extension()
                            .and_then(|value| value.to_str())
                            .unwrap_or("");
                        extension.eq_ignore_ascii_case("exe")
                            || extension.eq_ignore_ascii_case("sh")
                            || extension.eq_ignore_ascii_case("x86_64")
                    })
                    .map(|entry| entry.path().to_path_buf());
            let profile = executable.map(|command| LaunchProfile {
                working_directory: command.parent().map(PathBuf::from),
                command,
                arguments: Vec::new(),
                launcher_command: None,
            });
            result.push(Game::from_installation(
                title,
                "GOG",
                Installation {
                    path: install,
                    launcher: Launcher::Gog,
                    launch_profile: profile,
                    available: true,
                },
            ));
        }
    }
    result
}
