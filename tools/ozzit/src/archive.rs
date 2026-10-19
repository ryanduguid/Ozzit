//! OOXML parts are opaque bytes. No formula or cached value is evaluated here.
use crate::common::*;
use indexmap::IndexMap;
use std::{
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
};
pub type Parts = IndexMap<String, Vec<u8>>;
pub type ZipReader = zip::ZipArchive<BufReader<std::fs::File>>;
pub const BOOK: &str = "xl/workbook.xml";
pub fn open_zip(path: &Path) -> Result<ZipReader> {
    let file = std::fs::File::open(path).map_err(|e| io_error(&e, path))?;
    // Python's buffered open rejects a directory before parsing ZIP data.
    // Unix File::open accepts it, so retain EISDIR and its filename here.
    #[cfg(unix)]
    if file.metadata().map_err(|e| io_error(&e, path))?.is_dir() {
        return Err(io_error(&std::io::Error::from_raw_os_error(21), path));
    }
    zip::ZipArchive::new(BufReader::new(file)).map_err(|e| format!("invalid ZIP: {e}"))
}
pub fn member_text(zip: &mut ZipReader, name: &str) -> Result<String> {
    let mut index = None;
    for (i, member) in zip.file_names().enumerate() {
        if member.map_err(|e| format!("invalid ZIP name: {e}"))? == name {
            index = Some(i);
        }
    }
    let index =
        index.ok_or_else(|| format!("\"There is no item named '{name}' in the archive\""))?;
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
        let item = zip
            .by_index_raw(i)
            .map_err(|e| format!("invalid ZIP: {e}"))?;
        let name = item
            .name()
            .map_err(|e| format!("invalid ZIP name: {e}"))?
            .into_owned();
        decoded.insert(item.name_raw().to_vec(), name.clone());
        drop(item);
        if include(&name) {
            let mut item = zip.by_index(i).map_err(|e| format!("invalid ZIP: {e}"))?;
            let mut bytes = Vec::new();
            item.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            out.insert(name, bytes);
        }
    }
    let names = directory_names(zip.into_inner(), position, &decoded)?;
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
    let start = u32::try_from(out.len()).map_err(|_| "ZIP64 archive is too large")?;
    let len = u32::try_from(central.len()).map_err(|_| "ZIP64 directory is too large")?;
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
/// Appends one member's local header and data to `out` and its directory record to `central`.
fn write_member(out: &mut Vec<u8>, central: &mut Vec<u8>, name: &str, data: &[u8]) -> Result<()> {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(data).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    let size = u32::try_from(data.len()).map_err(|_| "ZIP64 member is too large")?;
    let csize =
        u32::try_from(compressed.len()).map_err(|_| "ZIP64 compressed member is too large")?;
    let offset = u32::try_from(out.len()).map_err(|_| "ZIP64 archive is too large")?;
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
