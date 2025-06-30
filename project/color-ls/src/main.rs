use chrono::{DateTime, Local};
use colored::{Color, Colorize};
use std::error::Error;
use std::fmt;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process;
use structopt::StructOpt;

// Custom error type for better error handling
#[derive(Debug)]
enum LsError {
    IoError(std::io::Error),
    InvalidFileName(String),
}

impl fmt::Display for LsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LsError::IoError(e) => write!(f, "IO error: {}", e),
            LsError::InvalidFileName(name) => write!(f, "Invalid filename: {}", name),
        }
}
}

fn count_directory_entries(path: &Path) -> Option<usize> {
    match fs::read_dir(path) {
        Ok(entries) => {
            // Count all entries (including hidden files for accuracy)
            let count = entries.count();
            Some(count)
        },
        Err(_) => None, // Permission denied or other error
    }
}

fn should_use_color(color_mode: &ColorMode) -> bool {
    match color_mode {
        ColorMode::Never => false,
        ColorMode::Always => true,
        ColorMode::Auto => {
            // Simple check for TTY - in a real implementation you might want to use
            // the `atty` crate for more robust detection
            std::env::var("TERM").is_ok() && std::env::var("NO_COLOR").is_err()
        },
    }
}

fn get_file_color(file: &FileInfo) -> Option<Color> {
    let mode = file.metadata.permissions().mode();

    // Check file type first
    if file.is_dir {
        return Some(Color::BrightCyan);
    }

    if file.is_symlink {
        return Some(Color::Red);
    }

    // Check if executable
    if mode & (libc::S_IXUSR | libc::S_IXGRP | libc::S_IXOTH) != 0 {
        return Some(Color::BrightGreen);
    }

    // Check by file extension
    if let Some(extension) = file.path.extension().and_then(|s| s.to_str()) {
        match extension.to_lowercase().as_str() {
            // Archive files
            "tar" | "tgz" | "arc" | "arj" | "taz" | "lha" | "lz4" | "lzh" | "lzma" | "tlz" |
            "txz" | "tzo" | "t7z" | "zip" | "z" | "dz" | "gz" | "lrz" | "lz" | "lzo" |
            "xz" | "zst" | "tzst" | "bz2" | "bz" | "tbz" | "tbz2" | "tz" | "deb" | "rpm" |
            "jar" | "war" | "ear" | "sar" | "rar" | "alz" | "ace" | "zoo" | "cpio" | "7z" |
            "rz" | "cab" | "wim" | "swm" | "dwm" | "esd" => Some(Color::Red),

            // Image files
            "jpg" | "jpeg" | "mjpg" | "mjpeg" | "gif" | "bmp" | "pbm" | "pgm" | "ppm" |
            "tga" | "xbm" | "xpm" | "tif" | "tiff" | "png" | "svg" | "svgz" | "mng" |
            "pcx" | "mov" | "mpg" | "mpeg" | "m2v" | "mkv" | "webm" | "ogm" | "mp4" |
            "m4v" | "mp4v" | "vob" | "qt" | "nuv" | "wmv" | "asf" | "rm" | "rmvb" |
            "flc" | "avi" | "fli" | "flv" | "gl" | "dl" | "xcf" | "xwd" | "yuv" | "cgm" |
            "emf" | "ogv" | "ogx" => Some(Color::Magenta),

            // Audio files
            "aac" | "au" | "flac" | "m4a" | "mid" | "midi" | "mka" | "mp3" | "mpc" |
            "ogg" | "ra" | "wav" | "oga" | "opus" | "spx" | "xspf" => Some(Color::Cyan),

            _ => Some(Color::BrightYellow),
        }
    } else {
        None
    }
}

fn list_directory(path: &Path, opt: &Opt) -> Result<(), LsError> {
    let mut entries = Vec::new();
    let use_color = should_use_color(&opt.color);
    let show_counts = !opt.no_dir_counts;

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_info = FileInfo::new(entry, show_counts)?;

            if should_show_file(&file_info.name, opt.all) {
                entries.push(file_info);
            }
        }
    } else {
        // Single file
        let file_info = FileInfo::from_path(path, show_counts)?;
        entries.push(file_info);
    }

    // Separate directories and files
    let mut directories: Vec<FileInfo> = Vec::new();
    let mut files: Vec<FileInfo> = Vec::new();

    for entry in entries {
        if entry.is_dir {
            directories.push(entry);
        } else {
            files.push(entry);
        }
    }

    // Sort both groups separately
    let sort_func = |entries: &mut Vec<FileInfo>| {
        if opt.sort_time {
            entries.sort_by_key(|f| f.metadata.modified().unwrap_or(std::time::UNIX_EPOCH));
        } else {
            entries.sort_by(|a, b| a.name.cmp(&b.name));
        }

        if opt.reverse {
            entries.reverse();
        }
    };

    sort_func(&mut directories);
    sort_func(&mut files);

    // Print entries with grouping
    if opt.long {
        // Print directories first
        for file in &directories {
            print_long_format(file, opt.human_readable, use_color, show_counts)?;
        }

        // Add line break between directories and files if both exist
        if !directories.is_empty() && !files.is_empty() {
            println!();
        }

        // Print files
        for file in &files {
            print_long_format(file, opt.human_readable, use_color, show_counts)?;
        }
    } else {
        // Short format with grouping
        let has_dirs = !directories.is_empty();
        let has_files = !files.is_empty();

        // Print spacer line
        println!();

        // Print directories first
        if has_dirs {
            for file in &directories {
                print_short_format(file, use_color, show_counts);
            }
            println!(); // End the directory line
        }

        // Add separation line if we have both directories and files
        //if has_dirs && has_files {
        //    println!(); // Empty line between groups
        //}

        // Print files
        if has_files {
            for file in &files {
                print_short_format(file, use_color, show_counts);
            }
            println!(); // End the files line
        }
        println!(); // Final spacer line
    }

    Ok(())
}


fn colorize_filename(file: &FileInfo, use_color: bool) -> String {
    if !use_color {
        return file.name.clone();
    }

    match get_file_color(file) {
        Some(color) => file.name.color(color).to_string(),
        None => file.name.clone(),
    }
}
fn format_filename_with_indicators(file: &FileInfo, use_color: bool, show_counts: bool) -> String {
    let colored_name = colorize_filename(file, use_color);

    if file.is_dir && show_counts {
        match file.dir_count {
            Some(count) => format!("{colored_name}({count})" ),
            None => format!("{}[?]", colored_name), // Permission denied or error
        }
    } else if file.is_dir {
        // Only show "/" when counts are disabled
        format!("{}/", colored_name)
    } else {
        colored_name
    }
}


impl Error for LsError {}

impl From<std::io::Error> for LsError {
    fn from(error: std::io::Error) -> Self {
        LsError::IoError(error)
    }
}

#[derive(Debug, Clone)]
enum ColorMode {
    Never,
    Always,
    Auto,
}

impl std::str::FromStr for ColorMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "never" | "no" | "none" => Ok(ColorMode::Never),
            "always" | "yes" | "force" => Ok(ColorMode::Always),
            "auto" | "tty" | "if-tty" => Ok(ColorMode::Auto),
            _ => Err(format!("Invalid color mode: {}", s)),
        }
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "ls", about = "A simple ls implementation in Rust with directory counts by default")]
struct Opt {
    /// Show hidden files (starting with .)
    #[structopt(short = "a", long = "all")]
    all: bool,

    /// Use long listing format
    #[structopt(short = "l", long = "long")]
    long: bool,

    /// Show human readable sizes
    #[structopt(short = "h", long = "human-readable")]
    human_readable: bool,

    /// Reverse sort order
    #[structopt(short = "r", long = "reverse")]
    reverse: bool,

    /// Sort by modification time
    #[structopt(short = "t", long = "time")]
    sort_time: bool,

    /// Control color output
    #[structopt(long = "color", default_value = "auto")]
    color: ColorMode,

    /// Disable directory entry counts (counts are shown by default)
    #[structopt(long = "no-dir-counts", short = "C")]
    no_dir_counts: bool,

    /// Paths to list
    #[structopt(parse(from_os_str))]
    paths: Vec<PathBuf>,
}

#[derive(Debug)]
struct FileInfo {
    name: String,
    path: PathBuf,
    metadata: fs::Metadata,
    is_dir: bool,
    is_symlink: bool,
    dir_count: Option<usize>, // Number of entries in directory (None if not a dir or unreadable)
}

impl FileInfo {
    fn new(entry: fs::DirEntry, count_dirs: bool) -> Result<Self, LsError> {
        let metadata = entry.metadata()?;
        let path = entry.path();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| LsError::InvalidFileName(format!("{:?}", entry.file_name())))?;

        let is_dir = path.is_dir();
        let is_symlink = path.is_symlink();
        let dir_count = if is_dir && count_dirs {
            count_directory_entries(&path)
        } else {
            None
        };

        Ok(FileInfo {
            name,
            path: path.clone(),
            metadata,
            is_dir,
            is_symlink,
            dir_count,
        })
    }

    fn from_path(path: &Path, count_dirs: bool) -> Result<Self, LsError> {
        let metadata = path.metadata()?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let is_dir = path.is_dir();
        let is_symlink = path.is_symlink();
        let dir_count = if is_dir && count_dirs {
            count_directory_entries(path)
        } else {
            None
        };

        Ok(FileInfo {
            name,
            path: path.to_path_buf(),
            metadata,
            is_dir,
            is_symlink,
            dir_count,
        })
    }
}

fn format_permissions(mode: u32) -> String {
    let file_type = match mode & libc::S_IFMT {
        libc::S_IFDIR => 'd',
        libc::S_IFLNK => 'l',
        libc::S_IFBLK => 'b',
        libc::S_IFCHR => 'c',
        libc::S_IFIFO => 'p',
        libc::S_IFSOCK => 's',
        _ => '-',
    };

    let user = format_permission_triplet(mode, libc::S_IRUSR, libc::S_IWUSR, libc::S_IXUSR);
    let group = format_permission_triplet(mode, libc::S_IRGRP, libc::S_IWGRP, libc::S_IXGRP);
    let other = format_permission_triplet(mode, libc::S_IROTH, libc::S_IWOTH, libc::S_IXOTH);

    format!("{}{}{}{}", file_type, user, group, other)
}

fn format_permission_triplet(mode: u32, read: u32, write: u32, execute: u32) -> String {
    let r = if mode & read != 0 { 'r' } else { '-' };
    let w = if mode & write != 0 { 'w' } else { '-' };
    let x = if mode & execute != 0 { 'x' } else { '-' };
    format!("{}{}{}", r, w, x)
}

fn format_size(size: u64, human_readable: bool) -> String {
    if !human_readable {
        return size.to_string();
    }

    const UNITS: &[&str] = &["B", "K", "M", "G", "T", "P"];
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{:.0}{}", size, UNITS[unit_index])
    } else {
        format!("{:.1}{}", size, UNITS[unit_index])
    }
}

fn print_long_format(file: &FileInfo, human_readable: bool, use_color: bool, show_counts: bool) -> Result<(), LsError> {
    let mode = file.metadata.permissions().mode();
    let nlink = file.metadata.nlink();
    let size = file.metadata.len();
    let modified: DateTime<Local> = DateTime::from(file.metadata.modified()?);

    let formatted_size = format_size(size, human_readable);
    let time_str = modified.format("%b %d %H:%M").to_string();
    let formatted_name = format_filename_with_indicators(file, use_color, show_counts);

    println!(
        "{} {:>3} {:>8} {} {}",
        format_permissions(mode),
        nlink,
        formatted_size,
        time_str,
        formatted_name
    );

    Ok(())
}

fn print_short_format(file: &FileInfo, use_color: bool, show_counts: bool) {
    let formatted_name = format_filename_with_indicators(file, use_color, show_counts);
    print!("{}  ", formatted_name);
}

fn should_show_file(name: &str, show_all: bool) -> bool {
    show_all || !name.starts_with('.')
}


fn run(opt: &Opt) -> Result<(), LsError> {
    let paths = if opt.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        opt.paths.clone()
    };

    for (i, path) in paths.iter().enumerate() {
        if paths.len() > 1 {
            if i > 0 {
                println!();
            }
            println!("{}:", path.display());
        }

        if let Err(e) = list_directory(path, opt) {
            eprintln!("ls: {}: {}", path.display(), e);
            continue;
        }
    }

    Ok(())
}

fn main() {
    let opt = Opt::from_args();

    if let Err(e) = run(&opt) {
        eprintln!("ls: {}", e);
        process::exit(1);
    }
}
