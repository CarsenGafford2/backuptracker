use crate::file_hasher;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct LocalTracker {
    json_file: String,
}

struct UpdateResult {
    index: usize,
    new_hash: Option<String>,
    remove_entry: bool,
    message: Option<String>,
}

impl LocalTracker {
    pub fn new() -> Self {
        LocalTracker {
            json_file: Self::get_json_path().to_str().unwrap().to_string(),
        }
    }

    fn get_json_path() -> std::path::PathBuf {
        let home_dir: std::path::PathBuf = dirs::home_dir().expect("Could not find home directory");
        let app_dir: std::path::PathBuf = home_dir.join(".safekp");
        if !app_dir.exists() {
            std::fs::create_dir_all(&app_dir).expect("Failed to create application directory");
        }
        app_dir.join("safekp_data.json")
    }

    pub fn track_folder(&self, folder_path: &str) {
        self.track_folder_with_backup(folder_path, folder_path);
    }

    pub fn track_folder_with_backup(&self, folder_path: &str, backup_folder_path: &str) {
        let folder = Path::new(folder_path);
        let backup_folder = Path::new(backup_folder_path);

        if !folder.exists() || !folder.is_dir() {
            println!("Invalid folder path: {}", folder_path);
            return;
        }

        let files: Vec<PathBuf> = walkdir::WalkDir::new(folder)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.path().to_path_buf())
            .collect();

        let progress_bar = ProgressBar::new(files.len() as u64);
        progress_bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.yellow/blue}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("=>-"),
        );
        progress_bar.set_message("Hashing files in parallel".to_string());

        let source_root = Self::normalize_location(folder, folder_path);
        let backup_root = Self::normalize_location(backup_folder, backup_folder_path);
        let tracked_entries: Vec<(String, serde_json::Value)> = files
            .par_iter()
            .filter_map(|file_path| {
                let relative_path = match file_path.strip_prefix(folder) {
                    Ok(path) => path,
                    Err(_) => {
                        progress_bar.inc(1);
                        return None;
                    }
                };

                let backup_location = backup_folder.join(relative_path);
                let result = Self::build_tracked_entry_with_root(
                    file_path,
                    &backup_location,
                    &source_root,
                    &backup_root,
                );
                progress_bar.inc(1);
                result
            })
            .collect();

        progress_bar.finish_with_message("Folder tracking complete".to_string());

        let mut tracked_files = self.read_tracked_files();
        if !tracked_files.is_empty() && !tracked_files[0].is_object() {
            tracked_files = Vec::new();
        }

        Self::normalize_locations_in_entries(&mut tracked_files);
        Self::merge_tracked_entries(&mut tracked_files, tracked_entries);
        self.write_tracked_files(&tracked_files);
    }

    fn build_tracked_entry(
        file_path: &Path,
        backup_location: &Path,
    ) -> Option<(String, serde_json::Value)> {
        if !file_path.exists() || !file_path.is_file() {
            return None;
        }

        let file_name = file_path.file_name()?.to_str()?.to_string();
        let file_path_str = file_path.to_str()?;
        let source_hash = file_hasher::FileHasher::new().hash_file(file_path_str)?;

        let source_location = Self::normalize_location(file_path, file_path_str);
        let backup_fallback = backup_location.to_string_lossy().to_string();
        let backup_location_string = Self::normalize_location(backup_location, &backup_fallback);
        let backup_hash = file_hasher::FileHasher::new().hash_file(&backup_location_string);

        let file_info = serde_json::json!({
            "name": file_name,
            "hash": source_hash,
            "location": source_location,
            "backup_location": backup_location_string,
            "backup_hash": backup_hash,
        });

        Some((
            file_info
                .get("location")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            file_info,
        ))
    }

    fn build_tracked_entry_with_root(
        file_path: &Path,
        backup_location: &Path,
        source_root: &str,
        backup_root: &str,
    ) -> Option<(String, serde_json::Value)> {
        if !file_path.exists() || !file_path.is_file() {
            return None;
        }

        let file_name = file_path.file_name()?.to_str()?.to_string();
        let file_path_str = file_path.to_str()?;
        let source_hash = file_hasher::FileHasher::new().hash_file(file_path_str)?;

        let source_location = Self::normalize_location(file_path, file_path_str);
        let backup_fallback = backup_location.to_string_lossy().to_string();
        let backup_location_string = Self::normalize_location(backup_location, &backup_fallback);
        let backup_hash = file_hasher::FileHasher::new().hash_file(&backup_location_string);

        let file_info = serde_json::json!({
            "name": file_name,
            "hash": source_hash,
            "location": source_location,
            "backup_location": backup_location_string,
            "backup_root": backup_root,
            "backup_hash": backup_hash,
            "source_root": source_root,
        });

        Some((
            file_info
                .get("location")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            file_info,
        ))
    }

    fn merge_tracked_entries(
        tracked_files: &mut Vec<serde_json::Value>,
        tracked_entries: Vec<(String, serde_json::Value)>,
    ) {
        let mut location_index = HashMap::new();

        for (index, tracked_file) in tracked_files.iter().enumerate() {
            if let Some(location) = tracked_file.get("location").and_then(|value| value.as_str()) {
                location_index.insert(Self::strip_windows_extended_prefix(location), index);
            }
        }

        for (location, file_info) in tracked_entries {
            if let Some(index) = location_index.get(&location).copied() {
                tracked_files[index] = file_info;
            } else {
                let new_index = tracked_files.len();
                tracked_files.push(file_info);
                location_index.insert(location, new_index);
            }
        }
    }

    fn normalize_location(path: &Path, fallback: &str) -> String {
        let location = path
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| fallback.to_string());

        Self::strip_windows_extended_prefix(&location)
    }

    fn strip_windows_extended_prefix(path: &str) -> String {
        if let Some(trimmed) = path.strip_prefix("\\\\?\\UNC\\") {
            return format!("\\\\{}", trimmed);
        }

        if let Some(trimmed) = path.strip_prefix("\\\\?\\") {
            return trimmed.to_string();
        }

        path.to_string()
    }

    fn normalize_locations_in_entries(entries: &mut [serde_json::Value]) {
        for entry in entries.iter_mut() {
            if let Some(location) = entry.get("location").and_then(|value| value.as_str()) {
                let normalized = Self::strip_windows_extended_prefix(location);
                if normalized != location {
                    entry["location"] = serde_json::Value::String(normalized);
                }
            }

            if let Some(backup_location) = entry
                .get("backup_location")
                .and_then(|value| value.as_str())
            {
                let normalized = Self::strip_windows_extended_prefix(backup_location);
                if normalized != backup_location {
                    entry["backup_location"] = serde_json::Value::String(normalized);
                }
            }

            if let Some(backup_root) = entry.get("backup_root").and_then(|value| value.as_str()) {
                let normalized = Self::strip_windows_extended_prefix(backup_root);
                if normalized != backup_root {
                    entry["backup_root"] = serde_json::Value::String(normalized);
                }
            }
        }
    }

    fn read_tracked_files(&self) -> Vec<serde_json::Value> {
        if !std::path::Path::new(&self.json_file).exists() {
            return Vec::new();
        }

        match std::fs::read_to_string(&self.json_file) {
            Ok(content) => {
                let content = content.trim();
                if content.is_empty() {
                    Vec::new()
                } else {
                    match serde_json::from_str::<serde_json::Value>(content) {
                        Ok(serde_json::Value::Array(arr)) => arr,
                        Ok(_) => Vec::new(),
                        Err(_) => Vec::new(),
                    }
                }
            }
            Err(_) => Vec::new(),
        }
    }

    fn write_tracked_files(&self, tracked_files: &[serde_json::Value]) {
        match serde_json::to_string_pretty(tracked_files) {
            Ok(serialized) => {
                if let Err(err) = std::fs::write(&self.json_file, serialized) {
                    eprintln!("Failed to write to JSON file: {}", err);
                }
            }
            Err(err) => eprintln!("Failed to serialize JSON: {}", err),
        }
    }

    pub fn track_file(&self, file_path: &str, backup_location: &str) {
        let file = Path::new(file_path);
        let backup_path = Path::new(backup_location);

        if !file.exists() || !file.is_file() {
            println!("Invalid file path: {}", file_path);
            return;
        }

        let tracked_entry = match Self::build_tracked_entry(file, backup_path) {
            Some(entry) => entry,
            None => {
                println!("Failed to hash file for tracking: {}", file_path);
                return;
            }
        };

        let mut tracked_files = self.read_tracked_files();
        if !tracked_files.is_empty() && !tracked_files[0].is_object() {
            tracked_files = Vec::new();
        }

        Self::normalize_locations_in_entries(&mut tracked_files);
        Self::merge_tracked_entries(&mut tracked_files, vec![tracked_entry]);
        self.write_tracked_files(&tracked_files);
    }

    pub fn untrack_file(&self, file_path: &str) {
        let file = std::path::Path::new(file_path);
        if file.exists() && file.is_file() {
            let location = Self::normalize_location(file, file_path);

            let mut tracked_files = self.read_tracked_files();

            Self::normalize_locations_in_entries(&mut tracked_files);

            let updated_files: Vec<serde_json::Value> = tracked_files
                .into_iter()
                .filter(|tracked_file| {
                    tracked_file
                        .get("location")
                        .and_then(|value| value.as_str())
                        .map_or(true, |tracked_location| {
                            let normalized_tracked_location = Self::strip_windows_extended_prefix(tracked_location);
                            normalized_tracked_location != location
                        })
                })
                .collect();

            self.write_tracked_files(&updated_files);

        } else {
            println!("Invalid file path: {}", file_path);
            return;
        }
    }

    pub fn update_backups(&self) {
        let mut tracked_files = self.read_tracked_files();

        if tracked_files.is_empty() {
            println!("No tracked files found.");
            return;
        }

        Self::normalize_locations_in_entries(&mut tracked_files);

        // First pass: scan for new files in tracked folders
        let mut updated_any_file = self.scan_and_backup_new_files(&mut tracked_files);

        // Second pass: update existing tracked files
        let progress_bar = ProgressBar::new(tracked_files.len() as u64);
        progress_bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.magenta/blue}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("=>-"),
        );
        progress_bar.set_message("Checking and updating backups in parallel".to_string());

        let results: Vec<UpdateResult> = tracked_files
            .iter()
            .enumerate()
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|(index, tracked_file)| {
                let source_location = match tracked_file.get("location").and_then(|value| value.as_str()) {
                    Some(value) => value.to_string(),
                    None => {
                        progress_bar.inc(1);
                        return UpdateResult {
                            index,
                            new_hash: None,
                            remove_entry: false,
                            message: Some("Skipping invalid tracked entry".to_string()),
                        };
                    }
                };

                let backup_location = match tracked_file.get("backup_location").and_then(|value| value.as_str()) {
                    Some(value) => value.to_string(),
                    None => {
                        progress_bar.inc(1);
                        return UpdateResult {
                            index,
                            new_hash: None,
                            remove_entry: false,
                            message: Some(format!("Skipping {} because backup location is missing", source_location)),
                        };
                    }
                };

                let source_path = Path::new(&source_location);
                let backup_path = Path::new(&backup_location);

                if !source_path.exists() || !source_path.is_file() {
                    progress_bar.inc(1);
                    return UpdateResult {
                        index,
                        new_hash: None,
                        remove_entry: true,
                        message: Some(format!("Tracked file no longer exists: {}", source_location)),
                    };
                }

                let source_hash = match file_hasher::FileHasher::new().hash_file(&source_location) {
                    Some(value) => value,
                    None => {
                        progress_bar.inc(1);
                        return UpdateResult {
                            index,
                            new_hash: None,
                            remove_entry: false,
                            message: Some(format!("Failed to hash tracked file: {}", source_location)),
                        };
                    }
                };

                let backup_hash = file_hasher::FileHasher::new().hash_file(&backup_location);
                if backup_hash.as_deref() == Some(source_hash.as_str()) {
                    progress_bar.inc(1);
                    return UpdateResult {
                        index,
                        new_hash: None,
                        remove_entry: false,
                        message: None,
                    };
                }

                if let Some(parent) = backup_path.parent() {
                    if let Err(err) = std::fs::create_dir_all(parent) {
                        progress_bar.inc(1);
                        return UpdateResult {
                            index,
                            new_hash: None,
                            remove_entry: false,
                            message: Some(format!("Failed to create backup directory for {}: {err}", backup_location)),
                        };
                    }
                }

                if let Err(err) = std::fs::copy(source_path, backup_path) {
                    progress_bar.inc(1);
                    return UpdateResult {
                        index,
                        new_hash: None,
                        remove_entry: false,
                        message: Some(format!("Failed to update backup file {}: {err}", backup_location)),
                    };
                }

                progress_bar.inc(1);
                UpdateResult {
                    index,
                    new_hash: Some(source_hash),
                    remove_entry: false,
                    message: None,
                }
            })
            .collect();

        let mut removed_indices = Vec::new();

        for result in results {
            if let Some(message) = result.message {
                println!("{}", message);
            }

            if result.remove_entry {
                if let Some(backup_location) = tracked_files[result.index]
                    .get("backup_location")
                    .and_then(|value| value.as_str())
                {
                    if let Err(err) = Self::remove_backup_artifact(Path::new(backup_location)) {
                        println!("Failed to remove backup file {}: {}", backup_location, err);
                    }
                }

                removed_indices.push(result.index);
                continue;
            }

            if let Some(new_hash) = result.new_hash {
                tracked_files[result.index]["hash"] = serde_json::Value::String(new_hash.clone());
                tracked_files[result.index]["backup_hash"] = serde_json::Value::String(new_hash);
                updated_any_file = true;
            }
        }

        if !removed_indices.is_empty() {
            removed_indices.sort_unstable();
            removed_indices.dedup();

            for index in removed_indices.into_iter().rev() {
                tracked_files.remove(index);
            }

            updated_any_file = true;
        }

        progress_bar.finish_with_message("Backup update scan complete".to_string());

        if updated_any_file {
            self.write_tracked_files(&tracked_files);
            println!("Backups updated successfully.");
        } else {
            println!("All backups are already up to date.");
        }
    }

    fn scan_and_backup_new_files(&self, tracked_files: &mut Vec<serde_json::Value>) -> bool {
        // Build a map of source_root to backup root and collect all unique source roots
        let mut source_root_info: HashMap<String, String> = HashMap::new();

        for entry in tracked_files.iter() {
            if let (Some(source_root), Some(location), Some(backup_location)) = (
                entry.get("source_root").and_then(|v| v.as_str()),
                entry.get("location").and_then(|v| v.as_str()),
                entry.get("backup_location").and_then(|v| v.as_str()),
            ) {
                let source_root_normalized = Self::strip_windows_extended_prefix(source_root);

                let backup_root = entry
                    .get("backup_root")
                    .and_then(|v| v.as_str())
                    .map(Self::strip_windows_extended_prefix)
                    .unwrap_or_else(|| {
                        Self::get_backup_root_for_source(backup_location, location, source_root)
                    });

                source_root_info
                    .entry(source_root_normalized)
                    .or_insert(backup_root);
            }
        }

        if source_root_info.is_empty() {
            return false;
        }

        let mut new_files_found = false;

        for (source_root, backup_root) in source_root_info {
            println!("Scanning for new files in: {}", source_root);

            let source_path = Path::new(&source_root);
            if !source_path.exists() || !source_path.is_dir() {
                continue;
            }

            // Get all files currently tracked from this source root
            let tracked_locations: std::collections::HashSet<String> = tracked_files
                .iter()
                .filter_map(|entry| {
                    entry.get("source_root")
                        .and_then(|v| v.as_str())
                        .and_then(|root| {
                            if Self::strip_windows_extended_prefix(root) == source_root {
                                entry.get("location").and_then(|v| v.as_str()).map(|l| Self::strip_windows_extended_prefix(l))
                            } else {
                                None
                            }
                        })
                })
                .collect();

            // Scan for new files
            let new_files: Vec<PathBuf> = walkdir::WalkDir::new(source_path)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_file())
                .filter(|entry| {
                    let normalized = Self::normalize_location(entry.path(), entry.path().to_str().unwrap_or(""));
                    !tracked_locations.contains(&normalized)
                })
                .map(|entry| entry.path().to_path_buf())
                .collect();

            if !new_files.is_empty() {
                println!("Found {} new file(s) in {}", new_files.len(), source_root);

                let progress_bar = ProgressBar::new(new_files.len() as u64);
                progress_bar.set_style(
                    ProgressStyle::with_template(
                        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
                    )
                    .unwrap()
                    .progress_chars("=>-"),
                );
                progress_bar.set_message("Backing up new files".to_string());

                for new_file in new_files {
                    let relative_path = match new_file.strip_prefix(source_path) {
                        Ok(path) => path,
                        Err(_) => {
                            progress_bar.inc(1);
                            continue;
                        }
                    };

                    let backup_location = Path::new(&backup_root).join(relative_path);

                    if let Some(parent) = backup_location.parent() {
                        if let Err(err) = std::fs::create_dir_all(parent) {
                            println!("Failed to create backup directory for {:?}: {}", new_file, err);
                            progress_bar.inc(1);
                            continue;
                        }
                    }

                    if let Err(err) = std::fs::copy(&new_file, &backup_location) {
                        println!("Failed to backup new file {:?}: {}", new_file, err);
                        progress_bar.inc(1);
                        continue;
                    }

                    // Add to tracked files
                    if let Some((_location, file_info)) = Self::build_tracked_entry_with_root(
                        &new_file,
                        &backup_location,
                        &source_root,
                        &backup_root,
                    ) {
                        tracked_files.push(file_info);
                        new_files_found = true;
                    }

                    progress_bar.inc(1);
                }

                progress_bar.finish_with_message("New files backed up".to_string());
            }
        }

        new_files_found
    }

    fn get_backup_root_for_source(backup_location: &str, source_location: &str, source_root: &str) -> String {
        if let Ok(source_path) = std::fs::canonicalize(source_location) {
            if let Ok(source_root_path) = std::fs::canonicalize(source_root) {
                if let Ok(rel_path) = source_path.strip_prefix(&source_root_path) {
                    if let Ok(backup_path) = std::fs::canonicalize(backup_location) {
                        let mut backup_root = backup_path.clone();
                        for _ in 0..rel_path.components().count() {
                            if let Some(parent) = backup_root.parent() {
                                backup_root = parent.to_path_buf();
                            } else {
                                break;
                            }
                        }
                        return backup_root.to_string_lossy().to_string();
                    }
                }
            }
        }
        backup_location.to_string()
    }

    fn remove_backup_artifact(path: &Path) -> std::io::Result<()> {
        if !path.exists() {
            return Ok(());
        }

        if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        }
    }

    pub fn update_file(&self, file_path: &str) {
        let file = std::path::Path::new(file_path);
        if file.exists() && file.is_file() {
            self.untrack_file(file_path);
            self.track_file(file_path, file_path);
        } else {
            println!("Invalid file path: {}", file_path);
            return;
        }
    }
}