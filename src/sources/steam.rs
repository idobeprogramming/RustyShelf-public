use crate::model::{Game, Installation, LaunchProfile, Launcher};
use crate::scan::{ScanControl, ScanEvent};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn discover(
    roots: &[PathBuf],
    control: &ScanControl,
    report: &mut dyn FnMut(ScanEvent),
) -> Vec<Game> {
    let mut steamapps = default_steamapps();
    steamapps.extend(
        roots
            .iter()
            .flat_map(|root| [root.join("SteamLibrary/steamapps"), root.join("steamapps")]),
    );
    discover_libraries(steamapps, control, report)
}

fn discover_libraries(
    mut steamapps: Vec<PathBuf>,
    control: &ScanControl,
    report: &mut dyn FnMut(ScanEvent),
) -> Vec<Game> {
    let mut result = Vec::new();
    let mut index = 0;
    while index < steamapps.len() {
        if control.is_cancelled() {
            break;
        }
        let library = steamapps[index].clone();
        index += 1;
        if !library.is_dir() {
            continue;
        }
        if let Some(vdf) = library
            .join("libraryfolders.vdf")
            .is_file()
            .then(|| super::read_or_report(&library.join("libraryfolders.vdf"), report))
            .flatten()
        {
            match library_paths(&vdf) {
                Ok(paths) => {
                    for candidate in paths {
                        if !steamapps.iter().any(|existing| {
                            crate::scan::path_is_within(existing, &candidate)
                                && crate::scan::path_is_within(&candidate, existing)
                        }) {
                            steamapps.push(candidate);
                        }
                    }
                }
                Err(error) => report(ScanEvent::Error(library.join("libraryfolders.vdf"), error)),
            }
        }
        let entries = match fs::read_dir(&library) {
            Ok(entries) => entries,
            Err(error) => {
                report(ScanEvent::Error(library.clone(), error.to_string()));
                continue;
            }
        };
        for entry in entries {
            if control.is_cancelled() {
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    report(ScanEvent::Error(library.clone(), error.to_string()));
                    continue;
                }
            };
            let path = entry.path();
            if is_manifest(&path) {
                match read_manifest(&path) {
                    Ok(Some(game)) => result.push(game),
                    Ok(None) => {}
                    Err(error) => report(ScanEvent::Error(path, error)),
                }
            }
        }
    }
    result
}

pub fn is_manifest(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name.eq_ignore_ascii_case("steamapps"))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                let name = name.to_ascii_lowercase();
                name.starts_with("appmanifest_") && name.ends_with(".acf")
            })
}

pub fn read_manifest(path: &Path) -> Result<Option<Game>, String> {
    let text = super::read_manifest_text(path)?;
    let tokens = vdf_tokens(&text)?;
    let fields = app_state_fields(&tokens)?;
    let field = |key: &str| fields.get(key).map(|value| (*value).to_owned());
    let name = field("name")
        .filter(|name| !name.trim().is_empty())
        .ok_or("Titre Steam manquant")?;
    let app_id = field("appid")
        .filter(|id| super::numeric_id(id))
        .ok_or("Identifiant Steam invalide")?;
    let install_dir = field("installdir").ok_or("Dossier Steam manquant")?;
    if !path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(format!("appmanifest_{app_id}.acf")))
    {
        return Err("Identifiant Steam différent du nom du manifeste".into());
    }
    let common = path
        .parent()
        .ok_or("Dossier Steam manquant")?
        .join("common");
    let install = super::relative_path(&common, &install_dir)?;
    if install == common
        || !Path::new(&install_dir.replace('\\', "/"))
            .components()
            .any(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return Err("Le manifeste Steam doit désigner un dossier de jeu".into());
    }
    if !install.is_dir() {
        return Ok(None);
    }
    Ok(Some(Game::from_installation(
        name,
        "Steam",
        Installation {
            path: install.clone(),
            launcher: Launcher::Steam,
            launch_profile: Some(LaunchProfile {
                command: PathBuf::new(),
                arguments: Vec::new(),
                working_directory: Some(install),
                launcher_command: Some(format!("steam://rungameid/{app_id}")),
            }),
            available: true,
        },
    )))
}

fn library_paths(text: &str) -> Result<Vec<PathBuf>, String> {
    // Read the whole document: several library records may share a single line.
    let tokens: Vec<_> = vdf_tokens(text)?
        .into_iter()
        .filter_map(|token| {
            if let VdfToken::Text(value) = token {
                Some(value)
            } else {
                None
            }
        })
        .collect();
    Ok(tokens
        .windows(2)
        .filter(|pair| pair[0] == "path" && !pair[1].trim().is_empty())
        .map(|pair| PathBuf::from(&pair[1]).join("steamapps"))
        .collect())
}

// Valve files can contain multiple fields on one line and escaped quotes.
#[derive(Debug, PartialEq)]
enum VdfToken {
    Text(String),
    Open,
    Close,
}

fn app_state_fields(tokens: &[VdfToken]) -> Result<std::collections::HashMap<&str, &str>, String> {
    if !matches!(tokens.first(), Some(VdfToken::Text(name)) if name == "AppState")
        || tokens.get(1) != Some(&VdfToken::Open)
    {
        return Err("Bloc AppState manquant dans le manifeste Steam".into());
    }
    let mut fields = std::collections::HashMap::new();
    let mut depth = 1;
    let mut index = 2;
    while index < tokens.len() {
        match &tokens[index] {
            VdfToken::Close => {
                depth -= 1;
                index += 1;
                if depth == 0 {
                    return if index == tokens.len() {
                        Ok(fields)
                    } else {
                        Err("Données inattendues après AppState".into())
                    };
                }
            }
            VdfToken::Text(key) => {
                match tokens.get(index + 1) {
                    Some(VdfToken::Text(value)) => {
                        if depth == 1 && fields.insert(key.as_str(), value.as_str()).is_some() {
                            return Err(format!("Champ Steam dupliqué : {key}"));
                        }
                    }
                    Some(VdfToken::Open) => depth += 1,
                    _ => return Err("Valeur manquante dans le manifeste Steam".into()),
                }
                index += 2;
            }
            VdfToken::Open => return Err("Bloc Steam sans nom".into()),
        }
    }
    Err("Bloc non fermé dans le manifeste Steam".into())
}

fn vdf_tokens(text: &str) -> Result<Vec<VdfToken>, String> {
    let mut chars = text.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'/') {
            for ch in chars.by_ref() {
                if ch == '\n' {
                    break;
                }
            }
        } else if ch == '"' {
            let mut value = String::new();
            let mut closed = false;
            while let Some(ch) = chars.next() {
                if ch == '"' {
                    closed = true;
                    break;
                }
                if ch == '\\' && matches!(chars.peek(), Some('"' | '\\')) {
                    value.push(chars.next().unwrap());
                } else {
                    value.push(ch);
                }
            }
            if !closed {
                return Err("Guillemet non fermé dans le manifeste Steam".into());
            }
            tokens.push(VdfToken::Text(value));
        } else if ch == '{' {
            tokens.push(VdfToken::Open);
        } else if ch == '}' {
            tokens.push(VdfToken::Close);
        } else if !ch.is_whitespace() && ch != '\u{feff}' {
            return Err("Caractère inattendu dans le manifeste Steam".into());
        }
    }
    Ok(tokens)
}

pub(super) fn default_steamapps() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        paths.extend([
            home.join(".steam/steam/steamapps"),
            home.join(".local/share/Steam/steamapps"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps"),
        ]);
    }
    #[cfg(windows)]
    {
        if let Some(program_files) = std::env::var_os("ProgramFiles(x86)") {
            paths.push(PathBuf::from(program_files).join("Steam/steamapps"));
        }
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            paths.push(PathBuf::from(program_files).join("Steam/steamapps"));
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn manifest_fields_must_belong_to_app_state_and_match_the_filename() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("steamapps");
        fs::create_dir_all(library.join("common/Été")).unwrap();
        let path = library.join("appmanifest_42.acf");
        let valid = r#""AppState" {
            "UserConfig" { "name" "Wrong title" "installdir" "Wrong folder" "appid" "999" }
            "appid" "42" "name" "Été {édition}" "installdir" "Été"
            // Braces inside comments and quoted strings are not structure: }
            "InstalledDepots" { "10" { "manifest" "123" } }
        }"#;
        fs::write(&path, format!("\u{feff}{valid}")).unwrap();
        let game = read_manifest(&path).unwrap().unwrap();
        assert_eq!(game.title, "Été {édition}");
        assert_eq!(game.installations[0].path, library.join("common/Été"));
        for invalid in [
            valid.replace("\"appid\" \"42\"", "\"appid\" \"43\""),
            valid.replace("\"name\" \"Été {édition}\"", ""),
            valid.replace("\"installdir\" \"Été\"", "\"installdir\" \".\""),
            valid.replace("\"appid\" \"42\"", "\"appid\" \"42\" \"appid\" \"42\""),
            valid.trim_end_matches('}').to_owned(),
            format!("{valid} }}"),
            valid.replace("\"AppState\"", "\"Other\""),
            r#""AppState" { "name" }"#.into(),
        ] {
            fs::write(&path, &invalid).unwrap();
            assert!(read_manifest(&path).is_err(), "{invalid}");
        }
    }

    #[test]
    fn compact_library_list_finds_all_drives_and_cycles_do_not_repeat_games() {
        let temp = tempdir().unwrap();
        let first = temp.path().join("Primary/steamapps");
        let second = temp.path().join("Disque Été/steamapps");
        let third = temp.path().join("Third/steamapps");
        for (index, library) in [&first, &second, &third].into_iter().enumerate() {
            fs::create_dir_all(library.join("common/Game")).unwrap();
            fs::write(library.join(format!("appmanifest_{}.acf", index + 1)), format!(
                "\"AppState\" {{ \"appid\" \"{}\" \"name\" \"Game {}\" \"installdir\" \"Game\" }}", index + 1, index
            )).unwrap();
        }
        let escaped = |library: &Path| {
            library
                .parent()
                .unwrap()
                .to_string_lossy()
                .replace('\\', "\\\\")
        };
        fs::write(
            first.join("libraryfolders.vdf"),
            format!(
                "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} }}",
                escaped(&second),
                escaped(&third)
            ),
        )
        .unwrap();
        fs::write(
            second.join("libraryfolders.vdf"),
            format!("\"path\" \"{}\"", escaped(&first)),
        )
        .unwrap();
        let games =
            discover_libraries(vec![first.clone()], &ScanControl::default(), &mut |event| {
                panic!("Unexpected event: {event:?}")
            });
        assert_eq!(games.len(), 3);
        // A broken list is reported, but the primary library is still scanned.
        fs::write(first.join("libraryfolders.vdf"), "\"path\" \"unterminated").unwrap();
        let mut errors = Vec::new();
        let games =
            discover_libraries(vec![first.clone()], &ScanControl::default(), &mut |event| {
                if let ScanEvent::Error(path, _) = event {
                    errors.push(path);
                }
            });
        assert_eq!(games.len(), 1);
        assert_eq!(errors, [first.join("libraryfolders.vdf")]);
    }
    #[test]
    fn handles_escaped_titles_and_rejects_invalid_ids_and_paths() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("steamapps");
        fs::create_dir_all(library.join("common/Game")).unwrap();
        let path = library.join("appmanifest_42.acf");
        let manifest = r#""AppState" { "appid" "42" "name" "The \"Game\"" "installdir" "Game" }"#;
        fs::write(&path, manifest).unwrap();
        assert_eq!(read_manifest(&path).unwrap().unwrap().title, "The \"Game\"");
        for text in [
            manifest.replace("\"42\"", "\"42?bad\""),
            manifest.replace("\"installdir\" \"Game\"", "\"installdir\" \"../outside\""),
            manifest.replace("\"installdir\" \"Game\"", "\"installdir\" \"C:/outside\""),
            "\"name\" \"unterminated".into(),
        ] {
            fs::write(&path, text).unwrap();
            assert!(read_manifest(&path).is_err());
        }
    }
    #[test]
    fn finds_local_manifest_in_secondary_library() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("SteamLibrary/steamapps");
        let installed = library.join("common/Été Game");
        fs::create_dir_all(&installed).unwrap();
        fs::write(library.join("appmanifest_42.acf"), "\"AppState\" {\n\"appid\" \"42\"\n\"name\" \"Été Game\"\n\"installdir\" \"Été Game\"\n}").unwrap();
        let found = [read_manifest(&library.join("appmanifest_42.acf"))
            .unwrap()
            .unwrap()];
        assert!(found
            .iter()
            .any(|game| game.title == "Été Game"
                && game.installations[0].launcher == Launcher::Steam));
    }
}
