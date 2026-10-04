//! A minimal, strict zip reader and writer for the library's small text files.
//!
//! The reader is the one place the app reads untrusted input, so it is deliberately narrow:
//! only stored and deflated entries, no encryption, no zip64, no multi-disk archives. Every
//! offset and length is bounds-checked, sizes are limited before anything is allocated, and the
//! decompressed size must match what the archive declares.
//!
//! The writer produces plain "stored" (uncompressed) archives that every zip tool can open.

use std::collections::HashSet;
use std::fmt;

use miniz_oxide::inflate::{TINFLStatus, decompress_to_vec_with_limit};

const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const END_SIGNATURE: u32 = 0x0605_4b50;
const UTF8_FLAG: u16 = 1 << 11;
const ENCRYPTED_FLAG: u16 = 1;
const STORED: u16 = 0;
const DEFLATED: u16 = 8;
/// 1980-01-01, the earliest zip date: archives are the same every time they are written.
const DOS_DATE: u16 = 0x0021;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_entries: usize,
    /// Largest uncompressed size of one file.
    pub max_file_bytes: usize,
    /// Largest uncompressed size of all files together.
    pub max_total_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZipError {
    NotAZip,
    Unsupported(&'static str),
    TooManyEntries,
    TooLarge,
    Corrupt(&'static str),
    /// The entry's name is not valid UTF-8 or uses a backslash.
    BadName,
    DuplicateEntry(String),
    CrcMismatch(String),
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAZip => write!(f, "this is not a zip file"),
            Self::Unsupported(what) => {
                write!(f, "this zip file uses {what}, which is not supported")
            }
            Self::TooManyEntries => write!(f, "the zip file has too many files"),
            Self::TooLarge => write!(f, "the zip file is too large"),
            Self::Corrupt(what) => write!(f, "the zip file is damaged ({what})"),
            Self::BadName => write!(f, "the zip file has a file name that is not allowed"),
            Self::DuplicateEntry(name) => write!(f, "the zip file contains \"{name}\" twice"),
            Self::CrcMismatch(name) => write!(f, "\"{name}\" in the zip file is damaged"),
        }
    }
}

impl std::error::Error for ZipError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipEntry {
    /// The name exactly as stored in the archive (not yet checked for path tricks).
    pub name: String,
    pub data: Vec<u8>,
}

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 == 1 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = crc_table();

fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |crc, &b| {
        CRC_TABLE[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8)
    })
}

// ---- writing ----------------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(crate) enum Method {
    Stored,
    #[cfg(test)]
    Deflated,
}

/// A zip archive of `entries` (name, bytes), uncompressed.
pub fn write_zip(entries: &[(&str, &[u8])]) -> Result<Vec<u8>, ZipError> {
    let entries: Vec<(&str, &[u8], Method)> = entries
        .iter()
        .map(|&(n, d)| (n, d, Method::Stored))
        .collect();
    build(&entries)
}

pub(crate) fn build(entries: &[(&str, &[u8], Method)]) -> Result<Vec<u8>, ZipError> {
    if entries.len() >= usize::from(u16::MAX) {
        return Err(ZipError::TooManyEntries);
    }
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for &(name, data, method) in entries {
        let (method_code, stored): (u16, std::borrow::Cow<[u8]>) = match method {
            Method::Stored => (STORED, data.into()),
            #[cfg(test)]
            Method::Deflated => (
                DEFLATED,
                miniz_oxide::deflate::compress_to_vec(data, 6).into(),
            ),
        };
        let too_large = u32::try_from(data.len()).is_err() || u32::try_from(stored.len()).is_err();
        let name_len = u16::try_from(name.len()).map_err(|_| ZipError::BadName)?;
        let offset = u32::try_from(out.len()).map_err(|_| ZipError::TooLarge)?;
        if too_large {
            return Err(ZipError::TooLarge);
        }
        let crc = crc32(data);
        let (compressed, size) = (stored.len() as u32, data.len() as u32);

        out.extend_from_slice(&LOCAL_SIGNATURE.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&UTF8_FLAG.to_le_bytes());
        out.extend_from_slice(&method_code.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // time
        out.extend_from_slice(&DOS_DATE.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&compressed.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra length
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&stored);

        central.extend_from_slice(&CENTRAL_SIGNATURE.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&UTF8_FLAG.to_le_bytes());
        central.extend_from_slice(&method_code.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&DOS_DATE.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&compressed.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&[0; 2]); // extra length
        central.extend_from_slice(&[0; 2]); // comment length
        central.extend_from_slice(&[0; 2]); // disk number
        central.extend_from_slice(&[0; 2]); // internal attributes
        central.extend_from_slice(&[0; 4]); // external attributes
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let central_offset = u32::try_from(out.len()).map_err(|_| ZipError::TooLarge)?;
    let central_size = u32::try_from(central.len()).map_err(|_| ZipError::TooLarge)?;
    out.extend_from_slice(&central);
    out.extend_from_slice(&END_SIGNATURE.to_le_bytes());
    out.extend_from_slice(&[0; 4]); // disk numbers
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&[0; 2]); // comment length
    Ok(out)
}

// ---- reading ----------------------------------------------------------------------------------

fn u16_at(bytes: &[u8], pos: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(pos..pos.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(bytes: &[u8], pos: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(pos..pos.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// Position of the end-of-central-directory record, searching back from the end of the file.
fn find_end_record(bytes: &[u8]) -> Option<usize> {
    let last = bytes.len().checked_sub(22)?;
    let first = last.saturating_sub(usize::from(u16::MAX));
    (first..=last).rev().find(|&pos| {
        u32_at(bytes, pos) == Some(END_SIGNATURE)
            && u16_at(bytes, pos + 20)
                .is_some_and(|comment| pos + 22 + usize::from(comment) <= bytes.len())
    })
}

/// Read every file in the archive. Folder entries are skipped. Nothing about the names is
/// trusted here beyond being valid text; callers must check paths themselves.
pub fn read_zip(bytes: &[u8], limits: &Limits) -> Result<Vec<ZipEntry>, ZipError> {
    let end = find_end_record(bytes).ok_or(ZipError::NotAZip)?;
    let corrupt = ZipError::Corrupt("truncated");
    let disk = u16_at(bytes, end + 4).ok_or(corrupt.clone())?;
    let central_disk = u16_at(bytes, end + 6).ok_or(corrupt.clone())?;
    let here = u16_at(bytes, end + 8).ok_or(corrupt.clone())?;
    let total = u16_at(bytes, end + 10).ok_or(corrupt.clone())?;
    let central_size = u32_at(bytes, end + 12).ok_or(corrupt.clone())?;
    let central_offset = u32_at(bytes, end + 16).ok_or(corrupt.clone())?;
    if disk != 0 || central_disk != 0 || here != total {
        return Err(ZipError::Unsupported("several disks"));
    }
    if total == u16::MAX || central_size == u32::MAX || central_offset == u32::MAX {
        return Err(ZipError::Unsupported("zip64"));
    }
    if usize::from(total) > limits.max_entries {
        return Err(ZipError::TooManyEntries);
    }
    let central_end = (central_offset as usize)
        .checked_add(central_size as usize)
        .filter(|&e| e <= bytes.len())
        .ok_or(ZipError::Corrupt("the file list is outside the file"))?;

    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut total_bytes = 0usize;
    let mut cursor = central_offset as usize;
    for _ in 0..total {
        let field = |offset: usize| cursor.checked_add(offset);
        if u32_at(bytes, cursor) != Some(CENTRAL_SIGNATURE) {
            return Err(ZipError::Corrupt("bad file list"));
        }
        let get16 = |o: usize| {
            field(o)
                .and_then(|p| u16_at(bytes, p))
                .ok_or(corrupt.clone())
        };
        let get32 = |o: usize| {
            field(o)
                .and_then(|p| u32_at(bytes, p))
                .ok_or(corrupt.clone())
        };
        let flags = get16(8)?;
        let method = get16(10)?;
        let crc = get32(16)?;
        let compressed_size = get32(20)? as usize;
        let size = get32(24)? as usize;
        let name_len = usize::from(get16(28)?);
        let extra_len = usize::from(get16(30)?);
        let comment_len = usize::from(get16(32)?);
        let local_offset = get32(42)? as usize;
        let name_start = cursor + 46;
        let name_bytes = bytes
            .get(name_start..name_start.checked_add(name_len).ok_or(corrupt.clone())?)
            .ok_or(corrupt.clone())?;
        cursor = name_start
            .checked_add(name_len)
            .and_then(|c| c.checked_add(extra_len))
            .and_then(|c| c.checked_add(comment_len))
            .filter(|&c| c <= central_end)
            .ok_or(ZipError::Corrupt("bad file list"))?;

        if flags & ENCRYPTED_FLAG != 0 {
            return Err(ZipError::Unsupported("encryption"));
        }
        if method != STORED && method != DEFLATED {
            return Err(ZipError::Unsupported(
                "a compression method other than deflate",
            ));
        }
        let name = std::str::from_utf8(name_bytes).map_err(|_| ZipError::BadName)?;
        if name.contains('\\') || name.contains('\0') {
            return Err(ZipError::BadName);
        }
        if !seen.insert(name.to_string()) {
            return Err(ZipError::DuplicateEntry(name.to_string()));
        }
        if name.ends_with('/') {
            if size != 0 {
                return Err(ZipError::Corrupt("a folder with content"));
            }
            continue;
        }
        if size > limits.max_file_bytes {
            return Err(ZipError::TooLarge);
        }
        total_bytes = total_bytes.checked_add(size).ok_or(ZipError::TooLarge)?;
        if total_bytes > limits.max_total_bytes {
            return Err(ZipError::TooLarge);
        }

        // The data starts after the local header, whose name and extra lengths may differ from
        // the file list's.
        if u32_at(bytes, local_offset) != Some(LOCAL_SIGNATURE) {
            return Err(ZipError::Corrupt("bad file header"));
        }
        let local_name = usize::from(u16_at(bytes, local_offset + 26).ok_or(corrupt.clone())?);
        let local_extra = usize::from(u16_at(bytes, local_offset + 28).ok_or(corrupt.clone())?);
        let data_start = local_offset
            .checked_add(30)
            .and_then(|s| s.checked_add(local_name))
            .and_then(|s| s.checked_add(local_extra))
            .ok_or(corrupt.clone())?;
        let raw = bytes
            .get(
                data_start
                    ..data_start
                        .checked_add(compressed_size)
                        .ok_or(corrupt.clone())?,
            )
            .ok_or(ZipError::Corrupt("file content is outside the file"))?;

        let data = if method == STORED {
            if compressed_size != size {
                return Err(ZipError::Corrupt("sizes do not match"));
            }
            raw.to_vec()
        } else {
            let data = decompress_to_vec_with_limit(raw, size).map_err(|e| {
                if e.status == TINFLStatus::HasMoreOutput {
                    ZipError::Corrupt("a file is larger than it says")
                } else {
                    ZipError::Corrupt("bad compressed data")
                }
            })?;
            if data.len() != size {
                return Err(ZipError::Corrupt("a file is smaller than it says"));
            }
            data
        };
        if crc32(&data) != crc {
            return Err(ZipError::CrcMismatch(name.to_string()));
        }
        entries.push(ZipEntry {
            name: name.to_string(),
            data,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const LIMITS: Limits = Limits {
        max_entries: 100,
        max_file_bytes: 10_000,
        max_total_bytes: 50_000,
    };

    fn entry(name: &str, data: &[u8]) -> ZipEntry {
        ZipEntry {
            name: name.to_string(),
            data: data.to_vec(),
        }
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn stored_archives_round_trip() {
        let zip = write_zip(&[
            ("songs.md", b"# Songs\n"),
            ("setlists/Café.md", "é".as_bytes()),
            ("empty.md", b""),
        ])
        .unwrap();
        let entries = read_zip(&zip, &LIMITS).unwrap();
        assert_eq!(
            entries,
            [
                entry("songs.md", b"# Songs\n"),
                entry("setlists/Café.md", "é".as_bytes()),
                entry("empty.md", b"")
            ]
        );
    }

    #[test]
    fn an_empty_archive_is_valid() {
        let zip = write_zip(&[]).unwrap();
        assert_eq!(read_zip(&zip, &LIMITS).unwrap(), []);
    }

    #[test]
    fn archives_are_the_same_every_time() {
        let a = write_zip(&[("a.md", b"x")]).unwrap();
        assert_eq!(a, write_zip(&[("a.md", b"x")]).unwrap());
    }

    #[test]
    fn deflated_entries_are_read() {
        let text = "Hotel California\n".repeat(200);
        let zip = build(&[
            ("songs.md", text.as_bytes(), Method::Deflated),
            ("b.md", b"plain", Method::Stored),
        ])
        .unwrap();
        assert!(zip.len() < text.len(), "it really is compressed");
        let entries = read_zip(&zip, &LIMITS).unwrap();
        assert_eq!(entries[0].data, text.as_bytes());
        assert_eq!(entries[1].data, b"plain");
    }

    #[test]
    fn folder_entries_are_skipped() {
        let zip = write_zip(&[("setlists/", b""), ("setlists/a.md", b"A")]).unwrap();
        assert_eq!(
            read_zip(&zip, &LIMITS).unwrap(),
            [entry("setlists/a.md", b"A")]
        );
        let bad = write_zip(&[("dir/", b"content")]).unwrap();
        assert!(matches!(read_zip(&bad, &LIMITS), Err(ZipError::Corrupt(_))));
    }

    #[test]
    fn things_that_are_not_zip_files_are_rejected() {
        for bytes in [&b""[..], b"hello world", &[0u8; 22], b"PK\x05\x06"] {
            assert_eq!(
                read_zip(bytes, &LIMITS),
                Err(ZipError::NotAZip),
                "{bytes:?}"
            );
        }
    }

    #[test]
    fn limits_apply_before_anything_big_is_made() {
        let zip = write_zip(&[("a.md", &[b'x'; 6_000]), ("b.md", &[b'x'; 6_000])]).unwrap();
        assert!(read_zip(&zip, &LIMITS).is_ok());
        let tight = Limits {
            max_file_bytes: 5_000,
            ..LIMITS
        };
        assert_eq!(read_zip(&zip, &tight), Err(ZipError::TooLarge));
        let total = Limits {
            max_total_bytes: 10_000,
            ..LIMITS
        };
        assert_eq!(read_zip(&zip, &total), Err(ZipError::TooLarge));
        let few = Limits {
            max_entries: 1,
            ..LIMITS
        };
        assert_eq!(read_zip(&zip, &few), Err(ZipError::TooManyEntries));
    }

    #[test]
    fn a_zip_bomb_is_rejected() {
        // 1 MB of zeros deflates to a few hundred bytes; the header lies about the size.
        let big = vec![0u8; 1_000_000];
        let mut zip = build(&[("bomb.md", &big, Method::Deflated)]).unwrap();
        // Honest header: refused for being over the limit.
        assert_eq!(read_zip(&zip, &LIMITS), Err(ZipError::TooLarge));
        // A lying header (claims 100 bytes in both places) must not be trusted either.
        let central = zip
            .windows(4)
            .position(|w| w == CENTRAL_SIGNATURE.to_le_bytes())
            .unwrap();
        for pos in [22, central + 24] {
            zip[pos..pos + 4].copy_from_slice(&100u32.to_le_bytes());
        }
        assert!(read_zip(&zip, &LIMITS).is_err());
    }

    #[test]
    fn encrypted_and_exotic_entries_are_refused() {
        let mut zip = write_zip(&[("a.md", b"A")]).unwrap();
        let central = zip
            .windows(4)
            .position(|w| w == CENTRAL_SIGNATURE.to_le_bytes())
            .unwrap();
        zip[central + 8] |= 1; // encrypted flag
        assert_eq!(
            read_zip(&zip, &LIMITS),
            Err(ZipError::Unsupported("encryption"))
        );
        let mut zip = write_zip(&[("a.md", b"A")]).unwrap();
        let central = zip
            .windows(4)
            .position(|w| w == CENTRAL_SIGNATURE.to_le_bytes())
            .unwrap();
        zip[central + 10] = 12; // bzip2
        assert!(matches!(
            read_zip(&zip, &LIMITS),
            Err(ZipError::Unsupported(_))
        ));
    }

    #[test]
    fn damaged_content_is_detected() {
        let mut zip = write_zip(&[("a.md", b"hello")]).unwrap();
        let data = zip.windows(5).position(|w| w == b"hello").unwrap();
        zip[data] = b'j';
        assert_eq!(
            read_zip(&zip, &LIMITS),
            Err(ZipError::CrcMismatch("a.md".to_string()))
        );
    }

    #[test]
    fn duplicate_names_and_backslashes_are_refused() {
        let dup = write_zip(&[("a.md", b"1"), ("a.md", b"2")]).unwrap();
        assert_eq!(
            read_zip(&dup, &LIMITS),
            Err(ZipError::DuplicateEntry("a.md".to_string()))
        );
        let slash = write_zip(&[("setlists\\a.md", b"1")]).unwrap();
        assert_eq!(read_zip(&slash, &LIMITS), Err(ZipError::BadName));
    }

    #[test]
    fn a_comment_after_the_archive_is_fine_and_trailing_garbage_is_not() {
        let mut zip = write_zip(&[("a.md", b"A")]).unwrap();
        let len = zip.len();
        zip[len - 2..].copy_from_slice(&5u16.to_le_bytes());
        zip.extend_from_slice(b"hello");
        assert_eq!(read_zip(&zip, &LIMITS).unwrap(), [entry("a.md", b"A")]);
    }

    fn which(tool: &str) -> bool {
        std::process::Command::new(tool).arg("-h").output().is_ok()
    }

    #[test]
    fn our_archives_open_in_the_real_unzip() {
        if !which("unzip") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.zip");
        std::fs::write(
            &path,
            write_zip(&[("songs.md", b"# Songs\n"), ("setlists/a.md", b"A\n")]).unwrap(),
        )
        .unwrap();
        let test = std::process::Command::new("unzip")
            .arg("-tq")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            test.status.success(),
            "{}",
            String::from_utf8_lossy(&test.stdout)
        );
        let extract = std::process::Command::new("unzip")
            .arg("-q")
            .arg(&path)
            .arg("-d")
            .arg(dir.path().join("x"))
            .status()
            .unwrap();
        assert!(extract.success());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("x/setlists/a.md")).unwrap(),
            "A\n"
        );
    }

    #[test]
    fn archives_made_by_the_real_zip_tool_are_read() {
        if !which("zip") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("Metronom");
        std::fs::create_dir_all(src.join("setlists")).unwrap();
        std::fs::write(src.join("songs.md"), "Hotel California\n".repeat(300)).unwrap();
        std::fs::write(src.join("setlists/friday.md"), "# Friday\n").unwrap();
        let out = std::process::Command::new("zip")
            .current_dir(dir.path())
            .args(["-qr", "made.zip", "Metronom"])
            .status()
            .unwrap();
        assert!(out.success());
        let bytes = std::fs::read(dir.path().join("made.zip")).unwrap();
        let entries = read_zip(&bytes, &LIMITS).unwrap();
        let by = |n: &str| entries.iter().find(|e| e.name == n).map(|e| e.data.clone());
        assert_eq!(
            by("Metronom/setlists/friday.md").as_deref(),
            Some(&b"# Friday\n"[..])
        );
        assert_eq!(
            by("Metronom/songs.md").map(|d| d.len()),
            Some(300 * "Hotel California\n".len())
        );
    }

    proptest! {
        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
            let _ = read_zip(&bytes, &LIMITS);
        }

        #[test]
        fn a_damaged_archive_never_panics_and_never_returns_wrong_data(
            cut in 0usize..400,
            flips in proptest::collection::vec((0usize..400, any::<u8>()), 0..4),
        ) {
            let text = "Hotel California\n".repeat(20);
            let original = build(&[("songs.md", text.as_bytes(), Method::Deflated), ("setlists/a.md", b"# A\n", Method::Stored)]).unwrap();
            let mut zip = original.clone();
            for (pos, value) in flips {
                if let Some(b) = zip.get_mut(pos) { *b ^= value | 1; }
            }
            zip.truncate(zip.len().saturating_sub(cut % (zip.len() + 1)));
            if let Ok(entries) = read_zip(&zip, &LIMITS) {
                // Whatever survives must be exactly what was written (the CRC guarantees it).
                for e in entries {
                    match e.name.as_str() {
                        "songs.md" => prop_assert_eq!(e.data, text.as_bytes()),
                        "setlists/a.md" => prop_assert_eq!(e.data, b"# A\n"),
                        _ => {}
                    }
                }
            }
        }

        #[test]
        fn any_files_round_trip(files in proptest::collection::vec(("[a-z]{1,8}(/[a-z]{1,8})?\\.md", proptest::collection::vec(any::<u8>(), 0..64)), 0..8)) {
            let mut seen = HashSet::new();
            let unique: Vec<(String, Vec<u8>)> = files.into_iter().filter(|(n, _)| seen.insert(n.clone())).collect();
            let refs: Vec<(&str, &[u8])> = unique.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
            let zip = write_zip(&refs).unwrap();
            let got = read_zip(&zip, &LIMITS).unwrap();
            let expected: Vec<ZipEntry> = unique.iter().map(|(n, d)| ZipEntry { name: n.clone(), data: d.clone() }).collect();
            prop_assert_eq!(got, expected);
        }
    }
}
