mod archiver;
mod processor;

use std::path::PathBuf;

use clap::Parser;
use log::LevelFilter;

// exist codes
const TARGET_NOT_EXISTS: i32 = 2;
const TARGET_NOT_DIR: i32 = 3;

/// SIT is a simple tool to take simple snapshots of a changing system.
/// This tool guarantees every file is backed up in the target zip in a stable state.
#[derive(Parser, Debug)]
struct SitArgs {
    /// Log Level for the application
    #[arg(short, long, default_value = "info", name = "logger")]
    log_level: LevelFilter,
    /// The directory to capture in the snapshot.
    #[arg(short, long, name = "target")]
    target_directory: String,
    /// Output file for the processed directory.
    /// The file is contained in a tar.gz format.
    #[arg(short, long, default_value = "output.tar.gz", name = "output")]
    output_file: String,
}

fn main() {
    let args = SitArgs::parse();

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{} [{}/{}]: {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(args.log_level)
        .chain(std::io::stdout())
        .apply()
        .expect("Failed to initialize logging");

    let target_path = PathBuf::from(args.target_directory);
    if !target_path.exists() {
        log::error!("Target directory does not exist: {}", target_path.display());
        std::process::exit(TARGET_NOT_EXISTS);
    }
    if !target_path.is_dir() {
        log::error!(
            "Target directory is not a directory: {}",
            target_path.display()
        );
        std::process::exit(TARGET_NOT_DIR);
    }
    processor::process_directory(target_path);
}
