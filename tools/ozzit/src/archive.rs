//! OOXML parts are opaque bytes. No formula or cached value is evaluated here.
use crate::{common::*, py};
use indexmap::IndexMap;
use std::{
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Arc,
};
pub type Parts = IndexMap<String, Vec<u8>>;
type Source = Cursor<Arc<[u8]>>;
/// The archive and its bytes, so a read can check the member's local header in
/// the same bytes it extracts from, as Python's open file does.
pub struct ZipReader {
    archive: zip::ZipArchive<Source>,
    bytes: Arc<[u8]>,
}
impl std::ops::Deref for ZipReader {
    type Target = zip::ZipArchive<Source>;
    fn deref(&self) -> &Self::Target {
        &self.archive
    }
}
impl std::ops::DerefMut for ZipReader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.archive
    }
}
pub const BOOK: &str = "xl/workbook.xml";
pub fn open_zip(path: &Path) -> Result<ZipReader> {
    let mut file = std::fs::File::open(path).map_err(|e| io_error(&e, path))?;
    // Python's buffered open rejects a directory before parsing ZIP data.
    // Unix File::open accepts it, so retain EISDIR and its filename here.
    #[cfg(unix)]
    if file.metadata().map_err(|e| io_error(&e, path))?.is_dir() {
        return Err(io_error(&std::io::Error::from_raw_os_error(21), path));
    }
    // shortcut: the whole archive is held in memory (workbooks are about 0.5 MB);
    // stream through a shared file handle if inputs ever grow large.
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| io_error(&e, path))?;
    let bytes: Arc<[u8]> = bytes.into();
    let archive = zip::ZipArchive::new(Cursor::new(Arc::clone(&bytes)))
        .map_err(|e| format!("invalid ZIP: {e}"))?;
    Ok(ZipReader { archive, bytes })
}
/// The general-purpose flags and filename of the header at `at`.
fn header_name(
    bytes: &[u8],
    at: u64,
    flags: usize,
    length: usize,
    name: usize,
) -> Result<(u16, &[u8])> {
    let field = |from: usize, len: usize| {
        usize::try_from(at)
            .ok()
            .and_then(|at| at.checked_add(from))
            .and_then(|from| bytes.get(from..from.checked_add(len)?))
            .ok_or_else(|| "invalid ZIP header: truncated".to_string())
    };
    let word = |from| field(from, 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    Ok((word(flags)?, field(name, usize::from(word(length)?))?))
}
/// `name.decode("utf-8")` when the UTF-8 flag is set, else `name.decode("cp437")`.
fn zip_name((flags, name): (u16, &[u8])) -> Result<String> {
    if flags & 0x800 != 0 {
        decode(name)
    } else {
        Ok(py::decode_cp437(name))
    }
}
/// The local and central header offsets of member `index`.
fn header_starts(zip: &mut ZipReader, index: usize) -> Result<(u64, u64)> {
    let item = zip
        .by_index_raw(index)
        .map_err(|e| format!("invalid ZIP: {e}"))?;
    Ok((item.header_start(), item.central_header_start()))
}
/// Member `index`'s name as Python's zipfile decodes it from the central directory.
fn central_name(zip: &mut ZipReader, index: usize) -> Result<String> {
    let (_, central_at) = header_starts(zip, index)?;
    zip_name(header_name(&zip.bytes, central_at, 8, 28, 46)?)
}
/// Python's `ZipFile.open` refuses a member whose local header names another file.
fn check_local_name(zip: &mut ZipReader, index: usize) -> Result<()> {
    let (local_at, _) = header_starts(zip, index)?;
    let name = central_name(zip, index)?;
    let local = header_name(&zip.bytes, local_at, 6, 26, 30)?;
    if zip_name(local)? == name {
        return Ok(());
    }
    Err(format!(
        "File name in directory {} and header {} differ.",
        py::repr_str(&name),
        py::repr_bytes(local.1)
    ))
}
pub fn member_text(zip: &mut ZipReader, name: &str) -> Result<String> {
    let mut index = None;
    for i in 0..zip.len() {
        if central_name(zip, i)? == name {
            index = Some(i);
        }
    }
    let index =
        index.ok_or_else(|| format!("\"There is no item named '{name}' in the archive\""))?;
    check_local_name(zip, index)?;
    let mut member = zip
        .by_index(index)
        .map_err(|e| format!("invalid ZIP: {e}"))?;
    let mut bytes = Vec::new();
    member.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    decode(&bytes)
}
pub fn read_parts(path: &Path) -> Result<Parts> {
    read_inventory(path).map(|(parts, _)| parts)
}
/// Retain central-directory order and duplicate entries as Python's namelist did.
pub fn read_inventory(path: &Path) -> Result<(Parts, Vec<String>)> {
    read_selected_inventory(path, |_| true)
}
pub fn read_selected_inventory(
    path: &Path,
    include: impl Fn(&str) -> bool,
) -> Result<(Parts, Vec<String>)> {
    let mut zip = open_zip(path)?;
    let position = zip.central_directory_start();
    let mut decoded = std::collections::HashMap::new();
    let mut out = Parts::new();
    for i in 0..zip.len() {
        let name = central_name(&mut zip, i)?;
        let raw = zip
            .by_index_raw(i)
            .map_err(|e| format!("invalid ZIP: {e}"))?
            .name_raw()
            .to_vec();
        decoded.insert(raw, name.clone());
        if include(&name) {
            check_local_name(&mut zip, i)?;
            let mut item = zip.by_index(i).map_err(|e| format!("invalid ZIP: {e}"))?;
            let mut bytes = Vec::new();
            item.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            out.insert(name, bytes);
        }
    }
    let names = directory_names(zip.archive.into_inner(), position, &decoded)?;
    Ok((out, names))
}
/// zip 9 keeps the last record for each raw filename. Read only the names
/// and lengths here; the library owns decoding, decompression and CRC checks.
fn directory_names(
    mut reader: impl Read + Seek,
    position: u64,
    decoded: &std::collections::HashMap<Vec<u8>, String>,
) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let length = reader.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    reader
        .seek(SeekFrom::Start(position))
        .map_err(|e| e.to_string())?;
    loop {
        let mut header = [0; 46];
        reader
            .read_exact(&mut header[..4])
            .map_err(|e| format!("invalid ZIP directory: {e}"))?;
        if &header[..4] != b"PK\x01\x02" {
            break;
        }
        reader
            .read_exact(&mut header[4..])
            .map_err(|e| format!("invalid ZIP directory header: {e}"))?;
        let size = |at| usize::from(u16::from_le_bytes([header[at], header[at + 1]]));
        let (name_length, extra_length, comment_length) = (size(28), size(30), size(32));
        let mut raw = vec![0; name_length];
        reader
            .read_exact(&mut raw)
            .map_err(|e| format!("invalid ZIP filename: {e}"))?;
        names.push(
            decoded
                .get(&raw)
                .ok_or("invalid ZIP directory name")?
                .clone(),
        );
        let position = reader
            .stream_position()
            .map_err(|e| e.to_string())?
            .checked_add(extra_length as u64)
            .and_then(|v| v.checked_add(comment_length as u64))
            .ok_or("invalid ZIP directory size")?;
        if position > length {
            return Err("invalid ZIP directory size".into());
        }
        reader
            .seek(SeekFrom::Start(position))
            .map_err(|e| e.to_string())?;
    }
    Ok(names)
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
/// Appends one member's local header and data to `out` and its directory record to `central`.
fn write_member(out: &mut Vec<u8>, central: &mut Vec<u8>, name: &str, data: &[u8]) -> Result<()> {
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
    }
}
