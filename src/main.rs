mod backup_engine;
mod file_hasher;
mod local_tracker;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        println!("No arguments provided");
        return;
    }

    if args[0] == "-v" || args[0] == "-version" && args.len() == 1 {
        print!("Safekp version 0.10.3");
    } else if args[0] == "-track" || args[0] == "-t" && args.len() == 3 {
        let backup_engine = backup_engine::BackupEngine::new();
        let tracker = local_tracker::LocalTracker::new();
        let path_to_track = std::path::Path::new(&args[1]);

        if let Some(backup_location) = backup_engine.backup(&args[1], &args[2]) {
            if path_to_track.is_dir() {
                tracker.track_folder_with_backup(&args[1], &backup_location);
            } else if path_to_track.is_file() {
                tracker.track_file(&args[1], &backup_location);
            }

            println!("File/Folder tracked and backed up successfully");
        }
    } else if args[0] == "-update" || args[0] == "-u" && args.len() == 1 {
        let tracker = local_tracker::LocalTracker::new();
        tracker.update_backups();
    } else if args[0] == "-backup" || args[0] == "-b" && args.len() == 3 {
        let backup_engine = backup_engine::BackupEngine::new();
        if let Some(backup_location) = backup_engine.backup(&args[1], &args[2]) {
            println!("Backup created at: {backup_location}");
        }
    } else if args[0] == "-help" || args[0] == "-h" && args.len() == 1 {
        println!("Usage: safekp [OPTIONS]");
        println!("Options:");
        println!("  -v, -version     Show version information");
        println!("  -t, -track       Track a file or folder and create a backup");
        println!("  -u, -update      Update existing backups");
        println!("  -b, -backup      Create a backup of a file or folder");
        println!("  -h, -help        Show this help message");
    }
}
