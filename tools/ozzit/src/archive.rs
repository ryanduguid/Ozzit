//! OOXML parts are opaque bytes. No formula or cached value is evaluated here.
use crate::common::*;
use indexmap::IndexMap;
use std::{
    io::{Cursor, Read, Write},
    path::Path,
};
pub type Parts = IndexMap<String, Vec<u8>>;
pub const BOOK: &str = "xl/workbook.xml";
pub fn read_parts(path: &Path) -> Result<Parts> {
    let data = read_bytes(path)?;
    let mut zip =
        zip::ZipArchive::new(Cursor::new(data)).map_err(|e| format!("invalid ZIP: {e}"))?;
    let mut out = Parts::new();
    for i in 0..zip.len() {
        let mut item = zip.by_index(i).map_err(|e| format!("invalid ZIP: {e}"))?;
        let name = item
            .name()
            .map_err(|e| format!("invalid ZIP name: {e}"))?
            .into_owned();
        let mut bytes = Vec::new();
        item.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        out.insert(name, bytes);
    }
    Ok(out)
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
        let data = &parts[name];
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(data).map_err(|e| e.to_string())?;
        let compressed = encoder.finish().map_err(|e| e.to_string())?;
        let size = u32::try_from(data.len()).map_err(|_| "ZIP64 member is too large")?;
        let csize =
            u32::try_from(compressed.len()).map_err(|_| "ZIP64 compressed member is too large")?;
        let offset = u32::try_from(out.len()).map_err(|_| "ZIP64 archive is too large")?;
        let namelen = u16::try_from(name.len()).map_err(|_| "ZIP member name is too long")?;
        let flags = if name.is_ascii() { 0 } else { 0x800 };
        let crc = crc32fast::hash(data);
        u32le(&mut out, 0x04034b50);
        u16le(&mut out, 20);
        u16le(&mut out, flags);
        u16le(&mut out, 8);
        u16le(&mut out, 0);
        u16le(&mut out, 0x5c21);
        u32le(&mut out, crc);
        u32le(&mut out, csize);
        u32le(&mut out, size);
        u16le(&mut out, namelen);
        u16le(&mut out, 0);
        out.extend(name.as_bytes());
        out.extend(compressed);
        u32le(&mut central, 0x02014b50);
        u16le(&mut central, 0x314);
        u16le(&mut central, 20);
        u16le(&mut central, flags);
        u16le(&mut central, 8);
        u16le(&mut central, 0);
        u16le(&mut central, 0x5c21);
        u32le(&mut central, crc);
        u32le(&mut central, csize);
        u32le(&mut central, size);
        u16le(&mut central, namelen);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u32le(&mut central, 0o644 << 16);
        u32le(&mut central, offset);
        central.extend(name.as_bytes());
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
pub fn write_deterministic(path: &Path, parts: &Parts) -> Result<()> {
    replace_atomically(path, &deterministic_bytes(parts)?)
}
