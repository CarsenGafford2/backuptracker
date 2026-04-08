use serde_json;
use crate::file_hasher;
use std::path::Path;

pub struct LocalTracker {
    json_file: String,
}

impl LocalTracker {
    pub fn new() -> Self {
        LocalTracker {
            json_file: Self::get_json_path().to_str().unwrap().to_string(),
        }
    }

    fn get_json_path() -> std::path::PathBuf {
        let home_dir: std::path::PathBuf = dirs::home_dir().expect("Could not find home directory");
        let app_dir: std::path::PathBuf = home_dir.join(".backup_tracker");
        if !app_dir.exists() {
            std::fs::create_dir_all(&app_dir).expect("Failed to create application directory");
        }
        app_dir.join("backup_tracker_data.json")
    }

    pub fn track_folder(&self, folder_path: &str) {
        self.track_folder_with_backup(folder_path, folder_path);
    }

    pub fn track_folder_with_backup(&self, folder_path: &str, backup_folder_path: &str) {
        let folder = std::path::Path::new(folder_path);
        let backup_folder = std::path::Path::new(backup_folder_path);
        if folder.exists() && folder.is_dir() {
            for entry in walkdir::WalkDir::new(folder) {
                if let Ok(entry) = entry {
                    if entry.path().is_file() {
                        let backup_location = entry
                            .path()
                            .strip_prefix(folder)
                            .ok()
                            .map(|relative_path| backup_folder.join(relative_path))
                            .unwrap_or_else(|| backup_folder.join(entry.path().file_name().unwrap()));

                        self.track_file(
                            entry.path().to_str().unwrap(),
                            backup_location.to_str().unwrap(),
                        );
                    }
                }
            }
        } else {
            println!("Invalid folder path: {}", folder_path);
            return;
        }
    }

    fn normalize_location(path: &Path, fallback: &str) -> String {
        let location = path
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| fallback.to_string());

        Self::strip_windows_extended_prefix(&location)
    }

    fn normalize_path_string(path: &str) -> String {
        Self::normalize_location(Path::new(path), path)
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
        let file = std::path::Path::new(file_path);
        if file.exists() && file.is_file() {
            let name = file.file_name().unwrap().to_str().unwrap();
            let hash = file_hasher::FileHasher::new().hash_file(file_path);
            let location = Self::normalize_location(file, file_path);
            let backup_location = Self::normalize_path_string(backup_location);
            let backup_hash = file_hasher::FileHasher::new().hash_file(&backup_location);

            let file_info = serde_json::json!({
                "name": name,
                "hash": hash,
                "location": location,
                "backup_location": backup_location,
                "backup_hash": backup_hash
            });

            let mut tracked_files = self.read_tracked_files();

            if !tracked_files.is_empty() && !tracked_files[0].is_object() {
                tracked_files = Vec::new();
            }

            Self::normalize_locations_in_entries(&mut tracked_files);

            for tracked_file in &mut tracked_files {
                if let Some(tracked_location) = tracked_file
                    .get("location")
                    .and_then(|value| value.as_str())
                {
                    let normalized_tracked_location = Self::strip_windows_extended_prefix(tracked_location);
                    if normalized_tracked_location == location {
                        tracked_file["name"] = serde_json::Value::String(name.to_string());
                        tracked_file["hash"] = serde_json::Value::String(hash.clone().unwrap_or_default());
                        tracked_file["backup_location"] = serde_json::Value::String(backup_location.clone());
                        tracked_file["backup_hash"] = serde_json::Value::String(backup_hash.clone().unwrap_or_default());

                        self.write_tracked_files(&tracked_files);

                        return;
                    }
                }
            }

            if tracked_files
                .last()
                .map(|value| value.is_object())
                .unwrap_or(false)
            {
                tracked_files.push(file_info);
            } else {
                tracked_files = vec![file_info];
            }

            self.write_tracked_files(&tracked_files);

        } else {
            println!("Invalid file path: {}", file_path);
            return;
        }
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

        let mut updated_any_file = false;

        for tracked_file in tracked_files.iter_mut() {
            let source_location = match tracked_file.get("location").and_then(|value| value.as_str()) {
                Some(value) => value.to_string(),
                None => continue,
            };

            let backup_location = match tracked_file
                .get("backup_location")
                .and_then(|value| value.as_str())
            {
                Some(value) => value.to_string(),
                None => continue,
            };

            let source_path = std::path::Path::new(&source_location);
            let backup_path = std::path::Path::new(&backup_location);

            if !source_path.exists() || !source_path.is_file() {
                println!("Tracked file no longer exists: {}", source_location);
                continue;
            }

            let source_hash = match file_hasher::FileHasher::new().hash_file(&source_location) {
                Some(value) => value,
                None => {
                    println!("Failed to hash tracked file: {}", source_location);
                    continue;
                }
            };

            let backup_hash = file_hasher::FileHasher::new().hash_file(&backup_location);
            let backup_is_current = backup_hash.as_deref() == Some(source_hash.as_str());

            if backup_is_current {
                continue;
            }

            if let Some(parent) = backup_path.parent() {
                if let Err(err) = std::fs::create_dir_all(parent) {
                    println!("Failed to create backup directory: {err}");
                    continue;
                }
            }

            if let Err(err) = std::fs::copy(source_path, backup_path) {
                println!("Failed to update backup file {}: {err}", backup_location);
                continue;
            }

            tracked_file["hash"] = serde_json::Value::String(source_hash.clone());
            tracked_file["backup_hash"] = serde_json::Value::String(source_hash);
            updated_any_file = true;
        }

        if updated_any_file {
            self.write_tracked_files(&tracked_files);
            println!("Backups updated successfully.");
        } else {
            println!("All backups are already up to date.");
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