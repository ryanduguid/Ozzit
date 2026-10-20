//! OOXML parts are opaque bytes. No formula or cached value is evaluated here.
use crate::{common::*, py};
use indexmap::IndexMap;
use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};
pub type Parts = IndexMap<String, Vec<u8>>;
pub const BOOK: &str = "xl/workbook.xml";

/// The open file, read at explicit offsets, as Python's `ZipFile` reads its one
/// file object.
struct Shared {
    file: File,
}
impl Shared {
    #[cfg(unix)]
    fn read_at(&self, buf: &mut [u8], at: u64) -> io::Result<usize> {
        std::os::unix::fs::FileExt::read_at(&self.file, buf, at)
    }
    #[cfg(windows)]
    fn read_at(&self, buf: &mut [u8], at: u64) -> io::Result<usize> {
        // Moves the handle's own pointer, which nothing here relies on.
        std::os::windows::fs::FileExt::seek_read(&self.file, buf, at)
    }
    fn len(&self) -> Result<u64> {
        Ok(self.file.metadata().map_err(|e| e.to_string())?.len())
    }
    /// Up to `len` bytes from `at`; fewer at the end of the file, as `fp.read` returns.
    fn bytes_at(&self, at: u64, len: u64) -> Result<Vec<u8>> {
        // A recorded size is no promise of that many bytes: allocate what is there.
        let len = len.min(self.len()?.saturating_sub(at));
        let len = usize::try_from(len).map_err(|_| "ZIP member is too large".to_string())?;
        let mut out = vec![0; len];
        let mut filled = 0;
        while filled < len {
            let from = at
                .checked_add(filled as u64)
                .ok_or("ZIP offset is too large")?;
            match self.read_at(&mut out[filled..], from) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        out.truncate(filled);
        Ok(out)
    }
}

/// One central-directory record, as Python's `ZipInfo` holds it.
struct Member {
    /// `orig_filename`, which the local header must match.
    orig: String,
    /// `filename`: the name `namelist()` reports and `read()` looks up.
    name: String,
    flags: u16,
    method: u16,
    crc: u32,
    compressed: u64,
    size: u64,
    /// `header_offset`, including any data prepended to the archive. As in
    /// Python, a wrong end record can make it negative.
    offset: i128,
    /// The next record's local header, or the directory: data may not run past it.
    end: i128,
}

/// An open archive read as Python's `zipfile` reads it: every central record in
/// order, names decoded by their own UTF-8 flag, the last record winning a name.
pub struct ZipReader {
    file: Shared,
    members: Vec<Member>,
    by_name: HashMap<String, usize>,
}

pub fn open_zip(path: &Path) -> Result<ZipReader> {
    let file = File::open(path).map_err(|e| io_error(&e, path))?;
    // Python's buffered open rejects a directory before parsing ZIP data.
    // Unix File::open accepts it, so retain EISDIR and its filename here.
    #[cfg(unix)]
    if file.metadata().map_err(|e| io_error(&e, path))?.is_dir() {
        return Err(io_error(&io::Error::from_raw_os_error(21), path));
    }
    let file = Shared { file };
    // `_RealGetContents`: the directory is the `size_cd` bytes before the end
    // record, wherever its recorded offset points; the difference is `concat`.
    let (size, offset, location) = end_record(&file)?;
    let start = i128::from(location) - i128::from(size);
    if start < 0 {
        return Err("Bad offset for central directory".into());
    }
    let concat = start - i128::from(offset);
    let directory = file.bytes_at(start as u64, size)?;
    let mut members = read_directory(&directory, size, concat)?;
    // Each record's data ends at the next local header.
    let mut order = (0..members.len()).collect::<Vec<_>>();
    order.sort_by_key(|&i| members[i].offset);
    let mut end = start;
    for &i in order.iter().rev() {
        members[i].end = end;
        end = members[i].offset;
    }
    let by_name = members
        .iter()
        .enumerate()
        .map(|(i, m)| (m.name.clone(), i))
        .collect();
    Ok(ZipReader {
        file,
        members,
        by_name,
    })
}

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}
fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn quad(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"))
}

const NOT_ZIP: &str = "File is not a zip file";

/// `_EndRecData`: the directory's size and offset and the end record's position.
/// zipfile ignores the disk numbers, entry counts and comment length here.
fn end_record(file: &Shared) -> Result<(u64, u64, u64)> {
    // `_RealGetContents` reports any read error here as not a ZIP file.
    let read = |at, len| file.bytes_at(at, len).map_err(|_| NOT_ZIP.to_string());
    let size = file.len().map_err(|_| NOT_ZIP.to_string())?;
    let at = size.checked_sub(22).ok_or(NOT_ZIP)?;
    let data = read(at, 22)?;
    let (record, location) =
        if data.len() == 22 && data.starts_with(b"PK\x05\x06") && data.ends_with(b"\0\0") {
            (data, at)
        } else {
            // An archive comment: the last signature in the final 64 KiB + 22 bytes.
            let from = size.saturating_sub(65_535 + 22);
            let data = read(from, 65_535 + 22)?;
            let at = data
                .windows(4)
                .rposition(|w| w == b"PK\x05\x06")
                .ok_or(NOT_ZIP)?;
            let record = data.get(at..at + 22).ok_or(NOT_ZIP)?.to_vec();
            (record, from + at as u64)
        };
    end_record64(
        file,
        long(&record, 12).into(),
        long(&record, 16).into(),
        location,
    )
}

/// `_EndRecData64`: a ZIP64 end record, found through its locator, replaces the
/// classic record's size, offset and position.
fn end_record64(file: &Shared, size: u64, offset: u64, location: u64) -> Result<(u64, u64, u64)> {
    let Some(locator) = location.checked_sub(20) else {
        return Ok((size, offset, location));
    };
    // A short read is Python's OSError, which `_RealGetContents` reports as not a ZIP file.
    let read = |at, len: u64| match file.bytes_at(at, len) {
        Ok(data) if data.len() as u64 == len => Ok(data),
        _ => Err(NOT_ZIP.to_string()),
    };
    let data = read(locator, 20)?;
    if !data.starts_with(b"PK\x06\x07") {
        return Ok((size, offset, location));
    }
    if long(&data, 4) != 0 || long(&data, 16) > 1 {
        return Err("zipfiles that span multiple disks are not supported".into());
    }
    let reloff = quad(&data, 8);
    let Some(at) = locator.checked_sub(56).filter(|&at| reloff <= at) else {
        return Err("Corrupt zip64 end of central directory locator".into());
    };
    let (mut data, mut extra) = (read(reloff, 56)?, at - reloff);
    if !data.starts_with(b"PK\x06\x06") && reloff != at {
        // Data prepended to the archive: assume no extensible data.
        (data, extra) = (read(at, 56)?, 0);
    }
    if !data.starts_with(b"PK\x06\x06") {
        return Err("Zip64 end of central directory record not found".into());
    }
    let (record_size, dir_size, dir_offset) = (quad(&data, 4), quad(&data, 40), quad(&data, 48));
    if u128::from(dir_offset) + u128::from(dir_size) != u128::from(reloff)
        || u128::from(record_size) + 12 != 56 + u128::from(extra)
    {
        return Err("Corrupt zip64 end of central directory record".into());
    }
    Ok((dir_size, dir_offset, at - extra))
}

/// Parses `size_cd` recorded bytes of directory as `_RealGetContents` does: a
/// record's name, extra field and comment are cut short at the end of the data
/// read, and the walk stops once the recorded lengths reach `size_cd`.
/// `io.BytesIO.read`: up to `len` bytes from `at`, which always advances.
fn take<'a>(data: &'a [u8], at: &mut usize, len: usize) -> &'a [u8] {
    let from = (*at).min(data.len());
    *at = at.saturating_add(len);
    &data[from..(*at).min(data.len())]
}

fn read_directory(directory: &[u8], size_cd: u64, concat: i128) -> Result<Vec<Member>> {
    let mut members = Vec::new();
    let (mut total, mut at) = (0u128, 0usize);
    let mut next = |len: usize| take(directory, &mut at, len);
    while total < u128::from(size_cd) {
        let header = next(46);
        if header.len() != 46 {
            return Err("Truncated central directory".into());
        }
        if header[..4] != *b"PK\x01\x02" {
            return Err("Bad magic number for central directory".into());
        }
        let lengths = [28, 30, 32].map(|at| usize::from(word(header, at)));
        let raw = next(lengths[0]);
        let flags = word(header, 8);
        let orig = zip_name(flags, raw)?;
        let extra = next(lengths[1]);
        next(lengths[2]);
        let version = header[6];
        if version > 63 {
            return Err(format!(
                "zip file version {}.{}",
                version / 10,
                version % 10
            ));
        }
        let mut member = Member {
            name: sanitize(&orig),
            orig,
            flags,
            method: word(header, 10),
            crc: long(header, 16),
            compressed: long(header, 20).into(),
            size: long(header, 24).into(),
            offset: 0,
            end: 0,
        };
        let mut offset = u64::from(long(header, 42));
        decode_extra(&mut member, &mut offset, extra, raw)?;
        member.offset = i128::from(offset) + concat;
        members.push(member);
        total += 46 + lengths.iter().map(|&n| n as u128).sum::<u128>();
    }
    Ok(members)
}

/// `name.decode("utf-8")` when the UTF-8 flag is set, else `name.decode("cp437")`.
fn zip_name(flags: u16, name: &[u8]) -> Result<String> {
    if flags & 0x800 != 0 {
        decode(name)
    } else {
        Ok(py::decode_cp437(name))
    }
}

/// `_sanitize_filename`: end at the first NUL; Windows separators become "/".
fn sanitize(name: &str) -> String {
    let name = name.split('\0').next().unwrap_or_default();
    if cfg!(windows) {
        name.replace('\\', "/")
    } else {
        name.to_owned()
    }
}

/// `ZipInfo._decodeExtra`: ZIP64 sizes and offset, and the Unicode Path field.
fn decode_extra(member: &mut Member, offset: &mut u64, mut extra: &[u8], raw: &[u8]) -> Result<()> {
    while extra.len() >= 4 {
        let (kind, length) = (word(extra, 0), usize::from(word(extra, 2)));
        if length + 4 > extra.len() {
            return Err(format!("Corrupt extra field {kind:04x} (size={length})"));
        }
        let data = &extra[4..length + 4];
        if kind == 0x0001 {
            let mut data = data;
            let fields: [(&str, &mut u64, u64); 3] = [
                ("File size", &mut member.size, 0xFFFF_FFFF),
                ("Compress size", &mut member.compressed, 0xFFFF_FFFF),
                ("Header offset", offset, 0xFFFF_FFFF),
            ];
            for (field, value, marker) in fields {
                if *value == marker {
                    let bytes = data
                        .get(..8)
                        .ok_or_else(|| format!("Corrupt zip64 extra field. {field} not found."))?;
                    *value = u64::from_le_bytes(bytes.try_into().expect("eight bytes"));
                    data = &data[8..];
                }
            }
        } else if kind == 0x7075 {
            if data.len() < 5 {
                return Err("Corrupt unicode path extra field (0x7075)".into());
            }
            if data[0] == 1 && long(data, 1) == crc32fast::hash(raw) {
                let name = std::str::from_utf8(&data[5..]).map_err(|_| {
                    "Corrupt unicode path extra field (0x7075): invalid utf-8 bytes".to_string()
                })?;
                if !name.is_empty() {
                    member.name = sanitize(name);
                }
            }
        }
        extra = &extra[length + 4..];
    }
    Ok(())
}

impl ZipReader {
    /// `namelist()`: every central record's name, duplicates included.
    pub fn names(&self) -> Vec<&str> {
        self.members.iter().map(|m| m.name.as_str()).collect()
    }

    /// `ZipFile.read(name)`: the last record with that name, checked as `open` checks it.
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let member = self
            .by_name
            .get(name)
            .map(|&i| &self.members[i])
            .ok_or_else(|| format!("\"There is no item named '{name}' in the archive\""))?;
        let start = self.data_start(member, name)?;
        self.contents(member, start)
    }

    /// `ZipFile.open`'s local-header checks, in its order; returns where the data starts.
    fn data_start(&self, member: &Member, name: &str) -> Result<u64> {
        // A file's `seek` refuses a negative position with EINVAL; past the end, the
        // header reads short.
        let offset = u64::try_from(member.offset.max(0)).unwrap_or(u64::MAX);
        if member.offset < 0 {
            return Err("[Errno 22] Invalid argument".into());
        }
        let header = self.file.bytes_at(offset, 30)?;
        if header.len() != 30 {
            return Err("Truncated file header".into());
        }
        if header[..4] != *b"PK\x03\x04" {
            return Err("Bad magic number for file header".into());
        }
        let local = self
            .file
            .bytes_at(offset.saturating_add(30), word(&header, 26).into())?;
        if member.flags & 0x20 != 0 {
            return Err("compressed patched data (flag bit 5)".into());
        }
        if member.flags & 0x40 != 0 {
            return Err("strong encryption (flag bit 6)".into());
        }
        if zip_name(word(&header, 6), &local)? != member.orig {
            return Err(format!(
                "File name in directory {} and header {} differ.",
                py::repr_str(&member.orig),
                py::repr_bytes(&local)
            ));
        }
        let start = [26, 28]
            .into_iter()
            .try_fold(offset.saturating_add(30), |at, field| {
                at.checked_add(word(&header, field).into())
            })
            .ok_or("ZIP offset is too large")?;
        // Python only warns when the next record starts at this one's own header.
        if i128::from(start) + i128::from(member.compressed) > member.end
            && member.end != member.offset
        {
            return Err(format!(
                "Overlapped entries: {} (possible zip bomb)",
                py::repr_str(&member.orig)
            ));
        }
        if member.flags & 1 != 0 {
            return Err(format!(
                "File {} is encrypted, password required for extraction",
                py::repr_str(name)
            ));
        }
        Ok(start)
    }

    /// The decompressed data, cut at `file_size` and checked against the CRC.
    fn contents(&self, member: &Member, start: u64) -> Result<Vec<u8>> {
        if ![0, 8].contains(&member.method) {
            return Err("That compression method is not supported".into());
        }
        let compressed = self.file.bytes_at(start, member.compressed)?;
        // `_read2` raises EOFError when a member's recorded data is missing,
        // even one whose `file_size` is 0.
        if compressed.is_empty() && member.compressed > 0 {
            return Err(format!(
                "{} ended before its compressed size",
                py::repr_str(&member.name)
            ));
        }
        let mut data = if member.method == 0 {
            compressed
        } else {
            let inflate_error = |e: io::Error| format!("Error while decompressing data: {e}");
            let mut decoder = flate2::read::DeflateDecoder::new(compressed.as_slice());
            // Python stops after `file_size` bytes; one more shows a longer stream.
            let mut out = Vec::new();
            (&mut decoder)
                .take(member.size.saturating_add(1))
                .read_to_end(&mut out)
                .map_err(inflate_error)?;
            // Python inflates up to 2,147,483,647 output bytes per read, so corrupt
            // data past `file_size` within that is still an error; a stream that
            // merely stops is not. Draining the rest of the stream is stricter:
            // corruption beyond that much output is refused here alone.
            match io::copy(&mut decoder, &mut io::sink()) {
                Err(e) if e.kind() != io::ErrorKind::UnexpectedEof => return Err(inflate_error(e)),
                _ => {}
            }
            out
        };
        if (data.len() as u64) < member.size {
            return Err(format!(
                "{} ended before its size",
                py::repr_str(&member.name)
            ));
        }
        data.truncate(usize::try_from(member.size).expect("no longer than read"));
        if crc32fast::hash(&data) != member.crc {
            return Err(format!(
                "Bad CRC-32 for file {}",
                py::repr_str(&member.name)
            ));
        }
        Ok(data)
    }
}

pub fn member_text(zip: &ZipReader, name: &str) -> Result<String> {
    decode(&zip.read(name)?)
}
pub fn read_parts(path: &Path) -> Result<Parts> {
    read_inventory(path).map(|(parts, _)| parts)
}
/// Retain central-directory order and duplicate entries as Python's namelist did.
pub fn read_inventory(path: &Path) -> Result<(Parts, Vec<String>)> {
    read_selected_inventory(path, |_| true)
}
/// The selected members, each read once by name as the Python tools read them, and
/// the complete name list.
pub fn read_selected_inventory(
    path: &Path,
    include: impl Fn(&str) -> bool,
) -> Result<(Parts, Vec<String>)> {
    let zip = open_zip(path)?;
    let mut out = Parts::new();
    for name in zip.names() {
        if include(name) && !out.contains_key(name) {
            out.insert(name.to_owned(), zip.read(name)?);
        }
    }
    let names = zip.names().into_iter().map(str::to_owned).collect();
    Ok((out, names))
}
pub fn text(parts: &Parts, name: &str) -> Result<String> {
    decode(
        parts
            .get(name)
            .ok_or_else(|| format!("\"There is no item named '{}' in the archive\"", name))?,
    )
}
pub fn read_book(parts: &Parts) -> Result<String> {
    text(parts, BOOK)
}
fn u16le(out: &mut Vec<u8>, n: u16) {
    out.extend(n.to_le_bytes());
}
fn u32le(out: &mut Vec<u8>, n: u32) {
    out.extend(n.to_le_bytes());
}
pub fn deterministic_bytes(parts: &Parts) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    let mut names = parts.keys().collect::<Vec<_>>();
    names.sort();
    if names.len() > 65535 {
        return Err("ZIP64 archive exceeds the workbook writer's member limit".into());
    }
    for name in names {
        write_member(&mut out, &mut central, name, &parts[name])?;
    }
    let start = classic(out.len()).ok_or("ZIP64 archive is too large")?;
    let len = classic(central.len()).ok_or("ZIP64 directory is too large")?;
    out.extend(central);
    u32le(&mut out, 0x06054b50);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, parts.len() as u16);
    u16le(&mut out, parts.len() as u16);
    u32le(&mut out, len);
    u32le(&mut out, start);
    u16le(&mut out, 0);
    Ok(out)
}
/// A size or offset Python's zipfile still writes without ZIP64 (`ZIP64_LIMIT`).
fn classic(n: usize) -> Option<u32> {
    u32::try_from(n).ok().filter(|&n| n <= 0x7fff_ffff)
}
/// `_open_to_write`'s preflight: Python also uses ZIP64 when `file_size * 1.05` exceeds the limit.
fn classic_preflight(n: usize) -> bool {
    n as f64 * 1.05 <= f64::from(0x7fff_ffff_u32)
}
/// Appends one member's local header and data to `out` and its directory record to `central`.
fn write_member(out: &mut Vec<u8>, central: &mut Vec<u8>, name: &str, data: &[u8]) -> Result<()> {
    if !classic_preflight(data.len()) {
        return Err("ZIP64 member is too large".into());
    }
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(data).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    let size = classic(data.len()).ok_or("ZIP64 member is too large")?;
    let csize = classic(compressed.len()).ok_or("ZIP64 compressed member is too large")?;
    let offset = classic(out.len()).ok_or("ZIP64 archive is too large")?;
    let namelen = u16::try_from(name.len()).map_err(|_| "ZIP member name is too long")?;
    let flags = if name.is_ascii() { 0 } else { 0x800 };
    let crc = crc32fast::hash(data);
    u32le(out, 0x04034b50);
    u16le(out, 20);
    u16le(out, flags);
    u16le(out, 8);
    u16le(out, 0);
    u16le(out, 0x5c21);
    u32le(out, crc);
    u32le(out, csize);
    u32le(out, size);
    u16le(out, namelen);
    u16le(out, 0);
    out.extend(name.as_bytes());
    out.extend(compressed);
    u32le(central, 0x02014b50);
    u16le(central, 0x314);
    u16le(central, 20);
    u16le(central, flags);
    u16le(central, 8);
    u16le(central, 0);
    u16le(central, 0x5c21);
    u32le(central, crc);
    u32le(central, csize);
    u32le(central, size);
    u16le(central, namelen);
    u16le(central, 0);
    u16le(central, 0);
    u16le(central, 0);
    u16le(central, 0);
    u32le(central, 0o644 << 16);
    u32le(central, offset);
    central.extend(name.as_bytes());
    Ok(())
}
pub fn write_deterministic(path: &Path, parts: &Parts) -> Result<()> {
    replace_atomically(path, &deterministic_bytes(parts)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn classic_fields_stop_where_python_switches_to_zip64() {
        assert_eq!(super::classic(0x7fff_ffff), Some(0x7fff_ffff));
        assert_eq!(super::classic(0x8000_0000), None);
        assert!(super::classic_preflight(2_045_222_520));
        assert!(!super::classic_preflight(2_045_222_521));
    }
}
