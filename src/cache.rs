//! Manage only regular files created by the photo pipeline.
use std::{
    fs, io,
    path::Path,
    time::{Duration, SystemTime},
};

pub const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub fn prune(dir: &Path, clear: bool) -> io::Result<u64> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !entry.file_type()?.is_file()
            || !name.ends_with(".bin")
            || name.len() != 20
            || !name.as_bytes()[..16].iter().all(u8::is_ascii_hexdigit)
        {
            continue;
        }
        let metadata = entry.metadata()?;
        files.push((metadata.modified()?, metadata.len(), entry.path()));
    }
    files.sort_by_key(|(modified, _, _)| *modified);
    let mut total = files.iter().map(|(_, size, _)| size).sum::<u64>();
    let mut removed = 0;
    for (modified, size, path) in files {
        let expired = SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default()
            > MAX_AGE;
        if clear || expired || total > MAX_BYTES {
            fs::remove_file(path)?;
            total = total.saturating_sub(size);
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eviction_and_clear_preserve_unrelated_files_and_directories() {
        let dir = std::env::temp_dir().join(format!("ttui-cache-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(dir.join("ffffffffffffffff.bin")).unwrap();
        fs::write(dir.join("notes.txt"), b"keep").unwrap();
        let file = fs::File::create(dir.join("aaaaaaaaaaaaaaaa.bin")).unwrap();
        file.set_len(MAX_BYTES + 1).unwrap();
        drop(file);
        assert_eq!(prune(&dir, false).unwrap(), 1);
        fs::write(dir.join("bbbbbbbbbbbbbbbb.bin"), b"cache").unwrap();
        assert_eq!(prune(&dir, true).unwrap(), 1);
        assert_eq!(fs::read(dir.join("notes.txt")).unwrap(), b"keep");
        assert!(dir.join("ffffffffffffffff.bin").is_dir());
        fs::remove_dir_all(dir).unwrap();
    }
}
