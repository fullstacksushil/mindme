//! The files opened lately, newest first. Kept as one path per line in `recent` in the config folder,
//! so they survive a restart.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

const KEEP: usize = 10;

/// A missing or unreadable file is an empty list: forgetting recent files loses nothing.
pub fn load(file: &Path) -> Vec<PathBuf> {
    fs::read_to_string(file).unwrap_or_default().lines().filter(|l| !l.is_empty()).map(PathBuf::from).take(KEEP).collect()
}

pub fn save(file: &Path, list: &[PathBuf]) -> io::Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(file, list.iter().map(|p| format!("{}\n", p.display())).collect::<String>())
}

/// Puts `path` first, drops its older entry, and keeps at most `KEEP`.
pub fn remember(list: &mut Vec<PathBuf>, path: &Path) {
    list.retain(|p| p != path);
    list.insert(0, path.to_owned());
    list.truncate(KEEP);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_no_repeats_at_most_ten() {
        let mut list = Vec::new();
        for i in 0..12 {
            remember(&mut list, Path::new(&format!("/m/{i}.md")));
        }
        remember(&mut list, Path::new("/m/5.md"));
        assert_eq!(list.len(), 10);
        assert_eq!(list[0], Path::new("/m/5.md"));
        assert_eq!(list[1], Path::new("/m/11.md"));
        assert_eq!(list.iter().filter(|p| p.ends_with("5.md")).count(), 1);
    }
}
