use std::{collections::HashMap, fs::File, io::Read, path::Path};

// Steam's binary app metadata is optional. Bound the read and fall back to
// the install directory if Steam has not cached usable metadata.
pub(super) fn read_titles(path: &Path) -> HashMap<u32, String> {
    const MAX_BYTES: u64 = 64 * 1024 * 1024;
    if !path
        .metadata()
        .is_ok_and(|meta| meta.is_file() && meta.len() <= MAX_BYTES)
    {
        return HashMap::new();
    }
    let Ok(file) = File::open(path) else {
        return HashMap::new();
    };
    let mut bytes = Vec::new();
    if file.take(MAX_BYTES + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > MAX_BYTES {
        return HashMap::new();
    }
    parse_titles(&bytes).unwrap_or_default()
}

fn parse_titles(mut bytes: &[u8]) -> Option<HashMap<u32, String>> {
    // Record sizes and string-table layout documented by SteamAppInfo.
    let original = bytes;
    let version = match read_u32(&mut bytes)? {
        0x0756_4427 => 39,
        0x0756_4428 => 40,
        0x0756_4429 => 41,
        _ => return None,
    };
    read_u32(&mut bytes)?; // Steam universe.
    let keys = if version == 41 {
        let offset =
            usize::try_from(u64::from_le_bytes(take(&mut bytes, 8)?.try_into().ok()?)).ok()?;
        let mut table = original.get(offset..)?;
        bytes = original.get(16..offset)?;
        let count = read_u32(&mut table)? as usize;
        if count > table.len() || count > 500_000 {
            return None;
        }
        let keys = (0..count)
            .map(|_| read_string(&mut table))
            .collect::<Option<Vec<_>>>()?;
        Some(keys)
    } else {
        None
    };
    let metadata_bytes = if version == 39 { 40 } else { 60 };
    let mut titles = HashMap::new();
    while let Some(app_id) = read_u32(&mut bytes) {
        if app_id == 0 {
            break;
        }
        let Some(size) = read_u32(&mut bytes) else {
            break;
        };
        let Some(record) = take(&mut bytes, size as usize) else {
            break;
        };
        if let Some(name) = record
            .get(metadata_bytes..)
            .and_then(|record| common_name(record, keys.as_deref()))
            .map(str::trim)
            .filter(|name| !name.is_empty() && name.len() <= 1024)
        {
            titles.insert(app_id, name.to_owned());
        }
    }
    Some(titles)
}

fn common_name<'a>(mut record: &'a [u8], keys: Option<&[&'a str]>) -> Option<&'a str> {
    if take(&mut record, 1)? != [0] || read_key(&mut record, keys)? != "appinfo" {
        return None;
    }
    let mut depth = 1;
    let mut in_common = false;
    loop {
        let kind = *take(&mut record, 1)?.first()?;
        if matches!(kind, 8 | 11) {
            if depth == 1 || (depth == 2 && in_common) {
                return None;
            }
            depth -= 1;
            continue;
        }
        let key = read_key(&mut record, keys)?;
        match kind {
            0 => {
                if depth == 1 {
                    in_common = key == "common";
                }
                depth += 1;
            }
            1 => {
                let value = read_string(&mut record)?;
                if in_common && depth == 2 && key == "name" {
                    return Some(value);
                }
            }
            2..=4 | 6 => {
                take(&mut record, 4)?;
            }
            7 | 10 => {
                take(&mut record, 8)?;
            }
            // Unsupported fields do not invalidate the other framed records.
            _ => return None,
        }
    }
}

fn read_key<'a>(bytes: &mut &'a [u8], keys: Option<&[&'a str]>) -> Option<&'a str> {
    match keys {
        Some(keys) => keys.get(read_u32(bytes)? as usize).copied(),
        None => read_string(bytes),
    }
}

fn read_string<'a>(bytes: &mut &'a [u8]) -> Option<&'a str> {
    let end = bytes.iter().position(|&byte| byte == 0)?;
    std::str::from_utf8(&take(bytes, end + 1)?[..end]).ok()
}

fn read_u32(bytes: &mut &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(take(bytes, 4)?.try_into().ok()?))
}

fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Option<&'a [u8]> {
    let (head, tail) = bytes.split_at_checked(length)?;
    *bytes = tail;
    Some(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(version: u32, id: u32, name: &str) -> Vec<u8> {
        let string_table = ["appinfo", "config", "name", "common"];
        let mut record = vec![0; if version == 39 { 40 } else { 60 }];
        let mut entry = |kind, key: usize, value: &str| {
            record.push(kind);
            if kind != 8 {
                if version == 41 {
                    record.extend((key as u32).to_le_bytes());
                } else {
                    record.extend(string_table[key].as_bytes());
                    record.push(0);
                }
            }
            if kind == 1 {
                record.extend(value.as_bytes());
                record.push(0);
            }
        };
        entry(0, 0, "");
        entry(0, 1, "");
        entry(1, 2, "Wrong section");
        entry(8, 0, "");
        entry(0, 3, "");
        entry(1, 2, name);
        record.extend([8, 8, 8]);

        let mut bytes = (0x0756_4400 | version).to_le_bytes().to_vec();
        bytes.extend(1u32.to_le_bytes());
        if version == 41 {
            bytes.extend((16u64 + 8 + record.len() as u64 + 4).to_le_bytes());
        }
        bytes.extend(id.to_le_bytes());
        bytes.extend((record.len() as u32).to_le_bytes());
        bytes.extend(record);
        bytes.extend(0u32.to_le_bytes());
        if version == 41 {
            bytes.extend((string_table.len() as u32).to_le_bytes());
            for key in string_table {
                bytes.extend(key.as_bytes());
                bytes.push(0);
            }
        }
        bytes
    }

    #[test]
    fn extracts_common_name_from_record_for_each_supported_cache_format() {
        for version in [39, 40, 41] {
            let mut bytes = fixture(version, 42, "Étoiles");
            assert_eq!(
                parse_titles(&bytes).unwrap().get(&42).map(String::as_str),
                Some("Étoiles")
            );
            bytes.truncate(bytes.len() / 2);
            assert!(!parse_titles(&bytes).unwrap_or_default().contains_key(&42));
        }
        let mut indexed = fixture(41, 42, "Étoiles");
        indexed[85..89].copy_from_slice(&u32::MAX.to_le_bytes()); // Invalid root-key index.
        assert!(!parse_titles(&indexed).unwrap().contains_key(&42));
    }

    #[test]
    fn local_appinfo_title_replaces_only_the_manifest_placeholder() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let library = root.join("steamapps");
        std::fs::create_dir_all(library.join("common/Steamed_Game")).unwrap();
        std::fs::create_dir(root.join("appcache")).unwrap();
        std::fs::write(
            root.join("appcache/appinfo.vdf"),
            fixture(41, 42, "Un jeu en été"),
        )
        .unwrap();
        let manifest = library.join("appmanifest_42.acf");
        for (name, expected) in [
            ("appid_42", "Un jeu en été"),
            ("Titre du manifeste", "Titre du manifeste"),
        ] {
            std::fs::write(&manifest, format!(
                "\"AppState\" {{ \"appid\" \"42\" \"name\" \"{name}\" \"installdir\" \"Steamed_Game\" }}"
            )).unwrap();
            let game = super::super::read_manifest(&manifest).unwrap().unwrap();
            assert_eq!(game.title, expected);
            assert_eq!(
                game.installations[0].path,
                library.join("common/Steamed_Game")
            );
            assert_eq!(
                game.installations[0]
                    .launch_profile
                    .as_ref()
                    .unwrap()
                    .launcher_command
                    .as_deref(),
                Some("steam://rungameid/42"),
            );
        }
    }
}
