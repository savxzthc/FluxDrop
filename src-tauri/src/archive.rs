use crate::file_utils::sanitize_filename;
use crate::share::{ArchiveEntrySource, SharePayload};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

const MAX_ARCHIVE_ENTRIES: usize = 100_000;

pub struct PreparedShare {
    pub payload: SharePayload,
    pub safe_file_name: String,
    pub original_file_name: String,
    pub file_size: u64,
    pub mime_type: String,
    pub file_count: usize,
    pub is_archive: bool,
}

pub fn prepare_share(paths: Vec<PathBuf>) -> Result<PreparedShare, String> {
    if paths.is_empty() {
        return Err("Choose at least one file or folder before starting a share.".to_string());
    }

    let mut canonical_paths = Vec::with_capacity(paths.len());
    let mut seen = HashSet::new();
    for path in paths {
        let canonical = std::fs::canonicalize(path)
            .map_err(|_| "One of the selected items could not be found.".to_string())?;
        if !seen.insert(canonical.clone()) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&canonical)
            .map_err(|_| "FluxDrop could not read a selected item.".to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("Symbolic links and junctions are not included in shares.".to_string());
        }
        if !metadata.is_file() && !metadata.is_dir() {
            return Err("Every selected item must be a regular file or folder.".to_string());
        }
        canonical_paths.push(canonical);
    }

    if canonical_paths.len() == 1 && canonical_paths[0].is_file() {
        return prepare_single_file(canonical_paths.remove(0));
    }

    prepare_archive(canonical_paths)
}

fn prepare_single_file(path: PathBuf) -> Result<PreparedShare, String> {
    let metadata = std::fs::metadata(&path)
        .map_err(|_| "FluxDrop could not read the selected file metadata.".to_string())?;
    std::fs::File::open(&path)
        .map_err(|_| "FluxDrop does not have permission to read the selected file.".to_string())?;
    let original_file_name = readable_file_name(&path)?;
    Ok(PreparedShare {
        safe_file_name: sanitize_filename(&original_file_name),
        original_file_name,
        file_size: metadata.len(),
        mime_type: mime_guess::from_path(&path)
            .first_or_octet_stream()
            .essence_str()
            .to_string(),
        payload: SharePayload::SingleFile { path },
        file_count: 1,
        is_archive: false,
    })
}

fn prepare_archive(paths: Vec<PathBuf>) -> Result<PreparedShare, String> {
    let single_folder = paths.len() == 1 && paths[0].is_dir();
    let archive_name = if single_folder {
        format!("{}.zip", sanitize_filename(&readable_file_name(&paths[0])?))
    } else {
        "FluxDrop-share.zip".to_string()
    };
    let mut entries = Vec::new();
    let mut used_paths = HashSet::new();
    let mut total_size = 0_u64;
    let mut file_count = 0_usize;

    for root in paths {
        let root_name = sanitize_filename(&readable_file_name(&root)?);
        if root.is_file() {
            add_file_entry(
                &mut entries,
                &mut used_paths,
                root.clone(),
                Path::new(&root_name),
                &mut total_size,
                &mut file_count,
            )?;
            continue;
        }

        for result in WalkDir::new(&root).follow_links(false).sort_by_file_name() {
            let entry = result.map_err(|err| format!("FluxDrop could not read a folder: {err}"))?;
            let metadata = entry
                .metadata()
                .map_err(|err| format!("FluxDrop could not read a folder entry: {err}"))?;
            if entry.file_type().is_symlink() {
                return Err(format!(
                    "Symbolic link '{}' was rejected from the archive.",
                    entry.path().display()
                ));
            }
            let relative = entry
                .path()
                .strip_prefix(&root)
                .map_err(|_| "FluxDrop could not construct a safe archive path.".to_string())?;
            if relative.as_os_str().is_empty() {
                continue;
            }
            let archive_path = safe_archive_path(Path::new(&root_name).join(relative).as_path())?;
            if metadata.is_dir() {
                let directory_path = unique_archive_path(
                    format!("{}/", archive_path.trim_end_matches('/')),
                    &mut used_paths,
                );
                entries.push(ArchiveEntrySource {
                    source_path: None,
                    archive_path: directory_path,
                    size: 0,
                    is_directory: true,
                });
            } else if metadata.is_file() {
                add_file_entry(
                    &mut entries,
                    &mut used_paths,
                    entry.path().to_path_buf(),
                    Path::new(&archive_path),
                    &mut total_size,
                    &mut file_count,
                )?;
            } else {
                return Err("A folder contains an unsupported filesystem entry.".to_string());
            }
            if entries.len() > MAX_ARCHIVE_ENTRIES {
                return Err(format!(
                    "This share contains more than {MAX_ARCHIVE_ENTRIES} archive entries."
                ));
            }
        }
    }

    if file_count == 0 {
        return Err("The selected folders do not contain any regular files.".to_string());
    }

    Ok(PreparedShare {
        payload: SharePayload::ZipArchive { entries },
        safe_file_name: archive_name.clone(),
        original_file_name: archive_name,
        file_size: total_size,
        mime_type: "application/zip".to_string(),
        file_count,
        is_archive: true,
    })
}

fn add_file_entry(
    entries: &mut Vec<ArchiveEntrySource>,
    used_paths: &mut HashSet<String>,
    source_path: PathBuf,
    archive_path: &Path,
    total_size: &mut u64,
    file_count: &mut usize,
) -> Result<(), String> {
    std::fs::File::open(&source_path)
        .map_err(|_| "FluxDrop does not have permission to read a selected file.".to_string())?;
    let metadata = std::fs::metadata(&source_path)
        .map_err(|_| "FluxDrop could not read a selected file.".to_string())?;
    let safe_path = safe_archive_path(archive_path)?;
    let safe_path = unique_archive_path(safe_path, used_paths);
    *total_size = total_size
        .checked_add(metadata.len())
        .ok_or_else(|| "The selected files are too large to represent safely.".to_string())?;
    *file_count += 1;
    entries.push(ArchiveEntrySource {
        source_path: Some(source_path),
        archive_path: safe_path,
        size: metadata.len(),
        is_directory: false,
    });
    Ok(())
}

fn readable_file_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "A selected item does not have a readable name.".to_string())
}

pub fn safe_archive_path(path: &Path) -> Result<String, String> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| "An archive entry name is not valid Unicode.".to_string())?;
                components.push(sanitize_filename(value));
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("Unsafe archive path traversal was rejected.".to_string());
            }
        }
    }
    if components.is_empty() {
        return Err("An archive entry has an empty path.".to_string());
    }
    Ok(components.join("/"))
}

fn unique_archive_path(path: String, used: &mut HashSet<String>) -> String {
    if used.insert(path.clone()) {
        return path;
    }
    let is_directory = path.ends_with('/');
    let trimmed = path.trim_end_matches('/');
    let (stem, extension) = match trimmed.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, format!(".{extension}")),
        _ => (trimmed, String::new()),
    };
    for index in 2.. {
        let candidate = format!(
            "{stem} ({index}){extension}{}",
            if is_directory { "/" } else { "" }
        );
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_directory_components() {
        assert!(safe_archive_path(Path::new("../../secret.txt")).is_err());
    }

    #[test]
    fn sanitizes_every_archive_component() {
        assert_eq!(
            safe_archive_path(Path::new("folder<script>/bad:name.txt")).expect("safe"),
            "folder_script_/bad_name.txt"
        );
    }

    #[test]
    fn prepares_folder_with_relative_structure() {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().join("photos");
        std::fs::create_dir_all(root.join("trip")).expect("mkdir");
        std::fs::write(root.join("trip").join("one.txt"), b"hello").expect("write");
        let prepared = prepare_share(vec![root]).expect("prepare");
        assert!(prepared.is_archive);
        assert_eq!(prepared.file_count, 1);
        let SharePayload::ZipArchive { entries } = prepared.payload else {
            panic!("expected archive");
        };
        assert!(entries
            .iter()
            .any(|entry| entry.archive_path == "photos/trip/one.txt"));
    }
}
