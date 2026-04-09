# Safekp

Safekp is a small command-line app made in Rust. It helps you make backups of files and folders, and it can also keep track of them so they can be updated later.

## What it does

- Makes a backup of a file or folder
- Tracks a file or folder after backing it up
- Updates tracked backups when the original file changes
- Removes missing files from the tracking list during update

## How to use it

The easiest way to use Safekp is to download the `.exe` file from the Releases section.

After you download it:

1. Put the `.exe` in a folder.
2. Add that folder to your `PATH`.
3. Open a terminal and run `safekp` with the option you want.

Example:

```bash
safekp -h
```

## Requirements

- Rust and Cargo installed, if you want to build from source

## Build from source

If you do not want to use the release `.exe`, you can compile the app yourself.

```bash
cargo build --release
```

The finished program will be in the `target/release` folder.

## Run from source

Use Cargo to run the app directly:

```bash
cargo run -- <option>
```

## Commands

### Show help

```bash
cargo run -- -h
```

### Show version

```bash
cargo run -- -v
```

### Create a backup

Backs up a file or folder to a destination folder.

```bash
cargo run -- -b <source> <destination>
```

Example:

```bash
cargo run -- -b C:\\Users\\Name\\Documents C:\\Backups
```

If the source is a folder, the app makes a new backup folder inside the destination. The folder name includes the source name and a timestamp.

### Track a file or folder

Tracks a file or folder and also makes a backup.

```bash
cargo run -- -t <source> <destination>
```

Example:

```bash
cargo run -- -t C:\\Users\\Name\\Documents\\notes.txt C:\\Backups
```

When a folder is tracked, the app saves the file list and hashes so it can check for changes later.

### Update tracked backups

Checks tracked items and updates backups when the source files change.

```bash
cargo run -- -u
```

## Where tracking data is stored

The app saves its tracking data in your home folder:

```text
.safekp/safekp_data.json
```

## Notes

- File and folder paths must be valid.
- The destination for backups should be a folder.
- Folder backups keep the folder structure.
- The app uses file hashes to check if a file changed.

## Project Files

- [src/main.rs](src/main.rs)
- [src/backup_engine.rs](src/backup_engine.rs)
- [src/local_tracker.rs](src/local_tracker.rs)
- [src/file_hasher.rs](src/file_hasher.rs)
