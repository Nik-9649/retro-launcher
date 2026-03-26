use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use walkdir::WalkDir;
use zip::ZipArchive;

use crate::db::Database;
use crate::models::{
    GameEntry, GameSourceRef, InstallState, Platform, SourceKind, default_emulator_for,
};

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "gb", "gbc", "gba", "cue", "chd", "m3u", "bin", "img", "iso", "nds", "nes", "sfc", "smc",
    "gen", "md", "smd", "n64", "z64", "v64",
];

/// Prefer the URL's basename when it is a `.zip` so we write an archive to disk (not misnamed `.nes` bytes).
pub fn download_filename_for_url(url: &str, catalog_filename: &str) -> String {
    let leaf = url
        .split('/')
        .next_back()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("");
    if !leaf.is_empty() && leaf.to_ascii_lowercase().ends_with(".zip") {
        leaf.to_string()
    } else {
        catalog_filename.to_string()
    }
}

pub fn path_looks_like_zip(path: &Path) -> Result<bool> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
    {
        return Ok(true);
    }
    let mut file = fs::File::open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let mut sig = [0u8; 4];
    let n = file.read(&mut sig)?;
    Ok(n >= 4 && sig == *b"PK\x03\x04")
}

/// If `downloaded` is a ZIP, extracts one `.nes` member into `download_root` and removes the archive.
pub fn resolve_downloaded_rom_path(
    downloaded: &Path,
    download_root: &Path,
    catalog_filename: &str,
) -> Result<PathBuf> {
    if !path_looks_like_zip(downloaded)? {
        return Ok(downloaded.to_path_buf());
    }

    let file = fs::File::open(downloaded)
        .with_context(|| format!("open {}", downloaded.display()))?;
    let mut archive = ZipArchive::new(file).context("read zip archive")?;

    let mut nes_entries: Vec<(usize, String)> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).with_context(|| format!("zip entry {i}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name.contains("..") || name.starts_with("__MACOSX/") {
            continue;
        }
        let path = Path::new(&name);
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if ext.eq_ignore_ascii_case("nes") {
            nes_entries.push((i, name));
        }
    }

    let index = pick_nes_zip_entry_index(&nes_entries, catalog_filename)?;
    let file = fs::File::open(downloaded)
        .with_context(|| format!("open {}", downloaded.display()))?;
    let mut archive = ZipArchive::new(file).context("read zip archive")?;
    let mut entry = archive
        .by_index(index)
        .with_context(|| format!("zip entry {index}"))?;

    let out_name = nes_output_filename(catalog_filename, entry.name())?;
    let requested_out_path = download_root.join(out_name);
    let out_path = if requested_out_path == downloaded {
        collision_safe_extract_path(download_root, &requested_out_path)?
    } else {
        requested_out_path
    };
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = fs::File::create(&out_path)
        .with_context(|| format!("create {}", out_path.display()))?;
    std::io::copy(&mut entry, &mut out).with_context(|| format!("extract {}", out_path.display()))?;

    fs::remove_file(downloaded).with_context(|| format!("remove {}", downloaded.display()))?;
    Ok(out_path)
}

fn collision_safe_extract_path(download_root: &Path, target_path: &Path) -> Result<PathBuf> {
    let file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .context("invalid output filename")?;
    let fallback = download_root.join(format!("{file_name}.extracted"));
    Ok(fallback)
}

fn pick_nes_zip_entry_index(entries: &[(usize, String)], catalog_filename: &str) -> Result<usize> {
    let expected_leaf = Path::new(catalog_filename)
        .file_name()
        .and_then(|n| n.to_str());
    if let Some(leaf) = expected_leaf {
        if let Some((i, _)) = entries.iter().find(|(_, path)| {
            Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(leaf))
        }) {
            return Ok(*i);
        }
    }
    match entries.len() {
        0 => anyhow::bail!("zip archive contains no .nes file"),
        1 => Ok(entries[0].0),
        _ => anyhow::bail!(
            "zip contains {} .nes files; add a catalog `filename` that matches one of them",
            entries.len()
        ),
    }
}

fn nes_output_filename(catalog_filename: &str, zip_entry_name: &str) -> Result<PathBuf> {
    if catalog_filename.to_ascii_lowercase().ends_with(".nes") {
        return Ok(PathBuf::from(
            Path::new(catalog_filename)
                .file_name()
                .context("invalid catalog filename")?,
        ));
    }
    Ok(PathBuf::from(
        Path::new(zip_entry_name)
            .file_name()
            .context("invalid zip entry name")?,
    ))
}

pub fn scan_rom_roots(
    db: &Database,
    roots: &[PathBuf],
    show_hidden: bool,
) -> Result<Vec<GameEntry>> {
    let mut discovered = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_entry(|entry| show_hidden || !is_hidden(entry.path()))
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase())
                .unwrap_or_default();
            if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
                continue;
            }
            let game = import_file(db, path, SourceKind::LocalScan, None, None)?;
            discovered.push(game);
        }
    }
    Ok(discovered)
}

pub fn import_file(
    db: &Database,
    path: &Path,
    source_kind: SourceKind,
    origin_url: Option<String>,
    origin_label: Option<String>,
) -> Result<GameEntry> {
    let bytes = fs::read(path)?;
    let hash = blake3::hash(&bytes).to_hex().to_string();
    let metadata = fs::metadata(path)?;
    if let Some(mut existing) = db.find_by_hash(&hash)? {
        let source_ref = GameSourceRef {
            kind: source_kind,
            label: origin_label.clone(),
            origin: origin_url.clone(),
        };
        if !existing.source_refs.contains(&source_ref) {
            existing.source_refs.push(source_ref);
        }
        if existing.rom_path.is_none() {
            existing.rom_path = Some(path.to_path_buf());
        }
        existing.install_state = resolve_install_state(existing.platform);
        existing.updated_at = Utc::now();
        db.upsert_game(&existing)?;
        return Ok(existing);
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let platform = Platform::from_extension(extension);
    let now = Utc::now();
    let game = GameEntry {
        id: format!("game:{hash}"),
        title: path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Unknown Title")
            .replace(['_', '-'], " "),
        filename: path
            .file_name()
            .and_then(|value| value.to_str())
            .map(ToString::to_string),
        platform,
        generation: platform.generation(),
        vibe_tags: platform.default_vibes(),
        source_kind,
        install_state: resolve_install_state(platform),
        managed_path: None,
        origin_url: origin_url.clone(),
        origin_label: origin_label.clone(),
        rom_path: Some(path.to_path_buf()),
        hash: Some(hash),
        emulator_kind: default_emulator_for(platform),
        checksum: None,
        size_bytes: Some(metadata.len()),
        play_count: 0,
        last_played_at: None,
        discovered_at: now,
        updated_at: now,
        source_refs: vec![GameSourceRef {
            kind: source_kind,
            label: origin_label,
            origin: origin_url,
        }],
        error_message: None,
        progress: None,
    };
    db.upsert_game(&game)?;
    Ok(game)
}

fn resolve_install_state(platform: Platform) -> InstallState {
    if default_emulator_for(platform).is_some() {
        InstallState::Ready
    } else {
        InstallState::Unsupported
    }
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use tempfile::tempdir;
    use zip::write::{SimpleFileOptions, ZipWriter};

    use super::*;

    #[test]
    fn imports_supported_rom() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("db.sqlite")).unwrap();
        let rom = dir.path().join("pokemon.gba");
        fs::write(&rom, b"demo-rom").unwrap();
        let game = import_file(&db, &rom, SourceKind::LocalScan, None, None).unwrap();
        assert_eq!(game.platform, Platform::GameBoyAdvance);
        assert_eq!(game.install_state, InstallState::Ready);
    }

    #[test]
    fn download_filename_uses_zip_basename_from_url() {
        assert_eq!(
            download_filename_for_url(
                "https://example.com/foo/bar/game.zip?x=1",
                "game.nes"
            ),
            "game.zip"
        );
        assert_eq!(
            download_filename_for_url("https://example.com/rom.nes", "rom.nes"),
            "rom.nes"
        );
    }

    #[test]
    fn extracts_nes_from_zip_and_removes_archive() {
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("bundle.zip");
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        zip.start_file("inner.nes", opts).unwrap();
        zip.write_all(b"ines-bytes").unwrap();
        zip.finish().unwrap();

        let out = resolve_downloaded_rom_path(&zip_path, dir.path(), "inner.nes").unwrap();
        assert_eq!(out, dir.path().join("inner.nes"));
        assert_eq!(fs::read(&out).unwrap(), b"ines-bytes");
        assert!(!zip_path.exists());
    }

    #[test]
    fn extracts_zip_safely_when_archive_path_matches_catalog_nes_name() {
        let dir = tempdir().unwrap();
        let downloaded_path = dir.path().join("Jungle Book, The (USA).nes");
        let file = fs::File::create(&downloaded_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        zip.start_file("Jungle Book, The (USA).nes", opts).unwrap();
        zip.write_all(b"ines-bytes").unwrap();
        zip.finish().unwrap();

        let out = resolve_downloaded_rom_path(
            &downloaded_path,
            dir.path(),
            "Jungle Book, The (USA).nes",
        )
        .unwrap();

        assert_eq!(fs::read(&out).unwrap(), b"ines-bytes");
        assert_ne!(out, downloaded_path);
        assert!(!downloaded_path.exists());
    }
}
