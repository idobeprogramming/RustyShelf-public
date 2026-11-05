use crate::model::{Game, Installation, LaunchProfile, Launcher};
use std::{
    fs,
    path::{Path, PathBuf},
};
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
            for entry in WalkDir::new(&install)
                .max_depth(2)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
            {
                if is_manifest(entry.path()) {
                    if let Ok(Some(game)) = read_manifest(entry.path()) {
                        result.push(game);
                    }
                }
            }
        }
    }
    result
}

pub fn is_manifest(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            let name = name.to_ascii_lowercase();
            name.starts_with("goggame-") && name.ends_with(".info")
        })
}

pub fn read_manifest(path: &Path) -> Result<Option<Game>, String> {
    let text = super::read_manifest_text(path)?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let title = json
        .get("name")
        .and_then(|value| value.as_str())
        .filter(|name| !name.trim().is_empty())
        .ok_or("Titre GOG manquant")?;
    let id = json
        .get("gameId")
        .and_then(|value| value.as_str())
        .filter(|id| super::numeric_id(id))
        .ok_or("Identifiant GOG invalide")?;
    let expected = format!("goggame-{id}.info");
    if !path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(&expected))
    {
        return Err("Identifiant GOG différent du nom du manifeste".into());
    }
    let install = path.parent().ok_or("Dossier GOG manquant")?.to_owned();
    // The primary game task is authoritative; never pick the first .exe in a folder.
    let task = json
        .get("playTasks")
        .and_then(|value| value.as_array())
        .and_then(|tasks| {
            tasks.iter().find(|task| {
                task.get("isPrimary").and_then(|value| value.as_bool()) == Some(true)
                    && task.get("type").and_then(|value| value.as_str()) == Some("FileTask")
                    && task
                        .get("category")
                        .and_then(|value| value.as_str())
                        .is_none_or(|category| matches!(category, "game" | "launcher"))
            })
        });
    let profile = if let Some(task) = task {
        let value = task
            .get("path")
            .and_then(|value| value.as_str())
            .ok_or("Commande GOG manquante")?;
        let command = super::relative_path(&install, value)?;
        let workdir = task
            .get("workingDir")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty());
        let working_directory = Some(match workdir {
            Some(value) => super::relative_path(&install, value)?,
            None => install.clone(),
        });
        let arguments = split_arguments(
            task.get("arguments")
                .and_then(|value| value.as_str())
                .unwrap_or(""),
        )?;
        let windows_binary_on_linux = cfg!(target_os = "linux")
            && command
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
        if command.is_file() && !windows_binary_on_linux {
            Some(LaunchProfile {
                command,
                arguments,
                working_directory,
                launcher_command: None,
            })
        } else {
            None
        }
    } else {
        None
    };
    Ok(Some(Game::from_installation(
        title,
        "GOG",
        Installation {
            path: install,
            launcher: Launcher::Gog,
            launch_profile: profile,
            available: true,
        },
    )))
}

// Split a Windows-style argument string without passing it to a shell.
fn split_arguments(text: &str) -> Result<Vec<String>, String> {
    let mut chars = text.chars().peekable();
    let mut result = Vec::new();
    let mut argument = String::new();
    let mut quoted = false;
    let mut started = false;
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() && !quoted {
            if started {
                result.push(std::mem::take(&mut argument));
                started = false;
            }
            continue;
        }
        started = true;
        match ch {
            '"' => quoted = !quoted,
            '\\' => {
                let mut count = 1;
                while chars.peek() == Some(&'\\') {
                    chars.next();
                    count += 1;
                }
                if chars.peek() == Some(&'"') {
                    argument.extend(std::iter::repeat_n('\\', count / 2));
                    chars.next();
                    if count % 2 == 0 {
                        quoted = !quoted;
                    } else {
                        argument.push('"');
                    }
                } else {
                    argument.extend(std::iter::repeat_n('\\', count));
                }
            }
            _ => argument.push(ch),
        }
    }
    if quoted {
        return Err("Guillemet non fermé dans les arguments GOG".into());
    }
    if started {
        result.push(argument);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selects_primary_task_and_keeps_quoted_arguments_without_running_it() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("Bin")).unwrap();
        fs::write(temp.path().join("Bin/run"), []).unwrap();
        fs::write(temp.path().join("helper.exe"), []).unwrap();
        let manifest = temp.path().join("goggame-42.info");
        let data = serde_json::json!({ "gameId": "42", "name": "Été", "playTasks": [
            {"type":"FileTask", "path":"helper.exe", "category":"document"},
            {"type":"FileTask", "isPrimary":true, "category":"game", "path":"Bin\\run", "workingDir":"Bin", "arguments":"--profile \"Mes jeux\""}
        ] });
        fs::write(&manifest, data.to_string()).unwrap();
        let game = read_manifest(&manifest).unwrap().unwrap();
        let profile = game.installations[0].launch_profile.as_ref().unwrap();
        assert_eq!(profile.command, temp.path().join("Bin/run"));
        assert_eq!(profile.arguments, ["--profile", "Mes jeux"]);
        assert_eq!(
            profile.working_directory.as_ref(),
            Some(&temp.path().join("Bin"))
        );
        assert!(game.confirmed);
        assert_eq!(
            split_arguments(r#"--file "C:\Mes jeux\save" "" "say \"hello\"""#).unwrap(),
            ["--file", "C:\\Mes jeux\\save", "", "say \"hello\""]
        );
        assert!(split_arguments("\"unfinished").is_err());
    }

    #[test]
    fn malformed_or_incomplete_gog_metadata_cannot_choose_an_arbitrary_executable() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("helper.exe"), []).unwrap();
        let manifest = temp.path().join("goggame-42.info");
        for text in [
            "broken",
            r#"{"gameId":"42"}"#,
            r#"{"name":"Game","gameId":"1"}"#,
        ] {
            fs::write(&manifest, text).unwrap();
            assert!(read_manifest(&manifest).is_err());
        }
        fs::write(&manifest, r#"{"name":"Game","gameId":"42"}"#).unwrap();
        assert!(read_manifest(&manifest).unwrap().unwrap().installations[0]
            .launch_profile
            .is_none());
    }
}
