use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
};

const SECTOR: usize = 2048;
const DIRECTORY_LIMIT: usize = 128 * 1024;

/// Inspect only the ISO9660 volume and the files used to identify a Sony game.
/// Raw CD sectors and other filesystems fall back to manual classification.
pub(super) fn detect(file: &mut File) -> io::Result<Option<&'static str>> {
    let size = file.metadata()?.len();
    let mut image = Image { file, size };
    let Some(root) = image.root_directory()? else {
        return Ok(None);
    };

    if let Some(config) = image.lookup(root, b"SYSTEM.CNF", false)? {
        let mut contents = [0; 8192];
        let len = config.len.min(contents.len());
        if image.read_at(config.offset, &mut contents[..len])?
            && contents[..len]
                .split(|&byte| byte == b'\n')
                .filter_map(|line| std::str::from_utf8(line).ok())
                .any(ps2_boot_line)
        {
            return Ok(Some("PlayStation 2"));
        }
    }

    let Some(game) = image.lookup(root, b"PSP_GAME", true)? else {
        return Ok(None);
    };
    let Some(system) = image.lookup(game, b"SYSDIR", true)? else {
        return Ok(None);
    };
    for name in [b"EBOOT.BIN".as_slice(), b"BOOT.BIN"] {
        if let Some(boot) = image.lookup(system, name, false)? {
            let mut magic = [0; 4];
            if boot.len >= magic.len()
                && image.read_at(boot.offset, &mut magic)?
                && matches!(&magic, b"~PSP" | b"\x7fELF")
            {
                return Ok(Some("PSP"));
            }
        }
    }
    Ok(None)
}

fn ps2_boot_line(line: &str) -> bool {
    let Some((key, value)) = line.split_once('=') else {
        return false;
    };
    key.trim().eq_ignore_ascii_case("BOOT2")
        && value
            .trim()
            .split_once(':')
            .is_some_and(|(device, command)| {
                device.eq_ignore_ascii_case("cdrom0") && !command.trim().is_empty()
            })
}

#[derive(Clone, Copy)]
struct Extent {
    offset: u64,
    len: usize,
}

struct Image<'a> {
    file: &'a mut File,
    size: u64,
}

impl Image<'_> {
    fn read_at(&mut self, offset: u64, bytes: &mut [u8]) -> io::Result<bool> {
        if offset.saturating_add(bytes.len() as u64) > self.size {
            return Ok(false);
        }
        self.file.seek(SeekFrom::Start(offset))?;
        match self.file.read_exact(bytes) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn root_directory(&mut self) -> io::Result<Option<Extent>> {
        let mut descriptor = [0; SECTOR];
        // ECMA-119 starts the volume descriptor sequence in sector 16. Allow
        // a boot record before the primary descriptor, with a fixed read limit.
        for sector in 16..32 {
            if !self.read_at((sector * SECTOR) as u64, &mut descriptor)?
                || &descriptor[1..7] != b"CD001\x01"
                || descriptor[0] == 255
            {
                break;
            }
            if descriptor[0] == 1 {
                // ISO stores the logical block size in both byte orders.
                if descriptor[128..132] != [0, 8, 8, 0] {
                    return Ok(None);
                }
                return Ok(parse_entry(&descriptor[156..190], b"\0", true, self.size));
            }
        }
        Ok(None)
    }

    fn lookup(&mut self, dir: Extent, name: &[u8], directory: bool) -> io::Result<Option<Extent>> {
        if dir.len > DIRECTORY_LIMIT {
            return Ok(None);
        }
        let mut block = [0; SECTOR];
        for block_offset in (0..dir.len).step_by(SECTOR) {
            let len = (dir.len - block_offset).min(SECTOR);
            let offset = dir.offset + block_offset as u64;
            if !self.read_at(offset, &mut block[..len])? {
                return Ok(None);
            }
            let mut entries = &block[..len];
            while let Some(&record_len) = entries.first() {
                // A zero byte pads to the next sector. Entries cannot straddle
                // sectors, and a truncated entry tells us nothing.
                if record_len < 34 {
                    break;
                }
                let Some((record, rest)) = entries.split_at_checked(usize::from(record_len)) else {
                    return Ok(None);
                };
                if let Some(extent) = parse_entry(record, name, directory, self.size) {
                    return Ok(Some(extent));
                }
                entries = rest;
            }
        }
        Ok(None)
    }
}

fn parse_entry(record: &[u8], expected: &[u8], directory: bool, size: u64) -> Option<Extent> {
    if record.len() < 34
        || usize::from(record[0]) != record.len()
        || record[1] != 0
        || (record[25] & 0x02 != 0) != directory
        || record[25] & 0x80 != 0
        || record[26..28] != [0, 0]
    {
        return None; // Unsupported extended attributes, interleaving or multiple extents.
    }
    let name = record.get(33..33 + usize::from(record[32]))?;
    let name = name.split(|&byte| byte == b';').next()?;
    if !name.eq_ignore_ascii_case(expected) {
        return None;
    }
    let sector = both_u32(&record[2..10])?;
    let len = both_u32(&record[10..18])? as usize;
    let offset = u64::from(sector) * SECTOR as u64;
    (len > 0 && offset + len as u64 <= size).then_some(Extent { offset, len })
}

fn both_u32(bytes: &[u8]) -> Option<u32> {
    let little = u32::from_le_bytes(bytes[..4].try_into().ok()?);
    let big = u32::from_be_bytes(bytes[4..].try_into().ok()?);
    (little == big).then_some(little)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn entry(name: &[u8], extent: u32, len: u32, directory: bool) -> Vec<u8> {
        let size = 33 + name.len() + usize::from(name.len().is_multiple_of(2));
        let mut record = vec![0; size];
        record[0] = size as u8;
        record[2..6].copy_from_slice(&extent.to_le_bytes());
        record[6..10].copy_from_slice(&extent.to_be_bytes());
        record[10..14].copy_from_slice(&len.to_le_bytes());
        record[14..18].copy_from_slice(&len.to_be_bytes());
        record[25] = if directory { 2 } else { 0 };
        record[32] = name.len() as u8;
        record[33..33 + name.len()].copy_from_slice(name);
        record
    }

    fn put(image: &mut [u8], sector: usize, data: &[u8]) {
        image[sector * SECTOR..sector * SECTOR + data.len()].copy_from_slice(data);
    }

    fn iso() -> Vec<u8> {
        let mut image = vec![0; 24 * SECTOR];
        put(&mut image, 16, b"\0CD001\x01");
        let descriptor = &mut image[17 * SECTOR..18 * SECTOR];
        descriptor[..7].copy_from_slice(b"\x01CD001\x01");
        descriptor[128..132].copy_from_slice(&[0, 8, 8, 0]);
        descriptor[156..190].copy_from_slice(&entry(b"\0", 20, SECTOR as u32, true));
        image
    }

    fn classify(bytes: &[u8]) -> io::Result<Option<&'static str>> {
        let mut file = tempfile::tempfile()?;
        file.write_all(bytes)?;
        detect(&mut file)
    }

    #[test]
    fn distinguishes_the_ps2_boot_key_from_other_iso_contents() {
        let mut image = iso();
        let valid = b"BOOT2 = cdrom0:\\SLES_000.00;1\r\n";
        put(
            &mut image,
            20,
            &entry(b"SYSTEM.CNF;1", 21, valid.len() as u32, false),
        );
        put(&mut image, 21, valid);
        assert_eq!(classify(&image).unwrap(), Some("PlayStation 2"));
        put(&mut image, 21, b"BOOT  = cdrom0:\\SLES_000.00;1\r\n");
        assert_eq!(classify(&image).unwrap(), None);
        put(&mut image, 21, valid);
        put(
            &mut image,
            20,
            &entry(b"FAKE__.CNF;1", 21, valid.len() as u32, false),
        );
        assert_eq!(classify(&image).unwrap(), None);
    }

    #[test]
    fn psp_game_requires_the_boot_path_and_executable_header() {
        let mut image = iso();
        put(&mut image, 20, &entry(b"PSP_GAME", 21, SECTOR as u32, true));
        put(&mut image, 21, &entry(b"SYSDIR", 22, SECTOR as u32, true));
        put(&mut image, 22, &entry(b"EBOOT.BIN;1", 23, 4, false));
        put(&mut image, 23, b"~PSP");
        assert_eq!(classify(&image).unwrap(), Some("PSP"));
        put(&mut image, 23, b"TEXT");
        assert_eq!(classify(&image).unwrap(), None);
        put(&mut image, 23, b"\x7fELF");
        put(
            &mut image,
            20,
            &entry(b"PSP_GAME", 21, SECTOR as u32, false),
        );
        assert_eq!(classify(&image).unwrap(), None);
    }

    #[test]
    fn truncated_unreadable_and_malformed_extents_do_not_classify() {
        let mut image = iso();
        put(
            &mut image,
            20,
            &entry(b"SYSTEM.CNF;1", u32::MAX, 8192, false),
        );
        assert_eq!(classify(&image).unwrap(), None);
        assert_eq!(classify(&image[..17 * SECTOR + 16]).unwrap(), None);

        let temp = tempfile::tempdir().unwrap();
        let mut write_only = File::create(temp.path().join("system.iso")).unwrap();
        write_only.write_all(&image).unwrap();
        assert!(detect(&mut write_only).is_err());
    }
}
