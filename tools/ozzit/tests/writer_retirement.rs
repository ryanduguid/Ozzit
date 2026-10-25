//! Original archive contracts use the independently frozen level-9 workbook bytes.
use ozzit::archive;
use std::path::Path;

fn written() -> (tempfile::TempDir, Vec<u8>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let source = root.join("ozzit.xlsx");
    let before = std::fs::read(&source).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("actual.xlsx");
    archive::write_deterministic(&target, &archive::read_parts(&source).unwrap()).unwrap();
    let actual = std::fs::read(&target).unwrap();
    assert_eq!(std::fs::read(source).unwrap(), before);
    assert_eq!(actual, before);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    (dir, actual)
}

#[test]
fn test_deterministic_writer_matches_explicit_level_9_reference() {
    // The original Python level-9 reference was independently proved equal to
    // the protected shipped workbook before deleting its helper implementation.
    written();
}

#[test]
fn test_deterministic_writer_uses_portable_file_metadata() {
    let (_dir, bytes) = written();
    let end = bytes.len() - 22;
    assert_eq!(&bytes[end..end + 4], b"PK\x05\x06");
    let mut offset = u32::from_le_bytes(bytes[end + 16..end + 20].try_into().unwrap()) as usize;
    let mut names = Vec::new();
    while offset < end {
        let record = &bytes[offset..];
        assert_eq!(&record[..4], b"PK\x01\x02");
        assert_eq!(record[5], 3);
        assert_eq!(u16::from_le_bytes(record[10..12].try_into().unwrap()), 8);
        assert_eq!(u16::from_le_bytes(record[12..14].try_into().unwrap()), 0);
        assert_eq!(
            u16::from_le_bytes(record[14..16].try_into().unwrap()),
            ((2026 - 1980) << 9) | 33
        );
        assert_eq!(
            u32::from_le_bytes(record[38..42].try_into().unwrap()) >> 16,
            0o644
        );
        let lengths = [28, 30, 32]
            .map(|at| u16::from_le_bytes(record[at..at + 2].try_into().unwrap()) as usize);
        names.push(String::from_utf8(record[46..46 + lengths[0]].to_vec()).unwrap());
        offset += 46 + lengths.iter().sum::<usize>();
    }
    assert_eq!(offset, end);
    assert_eq!(names.len(), 169);
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}
