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
        return Some(Color::Blue);
    }
    
    if file.is_symlink {
        return Some(Color::Cyan);
    }

    // Check if executable
    if mode & (libc::S_IXUSR | libc::S_IXGRP | libc::S_IXOTH) != 0 {
        return Some(Color::Green);
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
            
            _ => None,
        }
    } else {
        None
    }
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
#[structopt(name = "ls", about = "A simple ls implementation in Rust")]
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
}

impl FileInfo {
    fn new(entry: fs::DirEntry) -> Result<Self, LsError> {
        let metadata = entry.metadata()?;
        let path = entry.path();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| LsError::InvalidFileName(format!("{:?}", entry.file_name())))?;

        Ok(FileInfo {
            name,
            path: path.clone(),
            metadata,
            is_dir: path.is_dir(),
            is_symlink: path.is_symlink(),
        })
    }

    fn from_path(path: &Path) -> Result<Self, LsError> {
        let metadata = path.metadata()?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        Ok(FileInfo {
            name,
            path: path.to_path_buf(),
            metadata,
            is_dir: path.is_dir(),
            is_symlink: path.is_symlink(),
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

fn print_long_format(file: &FileInfo, human_readable: bool, use_color: bool) -> Result<(), LsError> {
    let mode = file.metadata.permissions().mode();
    let nlink = file.metadata.nlink();
    let size = file.metadata.len();
    let modified: DateTime<Local> = DateTime::from(file.metadata.modified()?);

    let formatted_size = format_size(size, human_readable);
    let time_str = modified.format("%b %d %H:%M").to_string();
    let colored_name = colorize_filename(file, use_color);

    println!(
        "{} {:>3} {:>8} {} {}{}",
        format_permissions(mode),
        nlink,
        formatted_size,
        time_str,
        colored_name,
        if file.is_dir { "/" } else { "" }
    );

    Ok(())
}

fn print_short_format(file: &FileInfo, use_color: bool) {
    let colored_name = colorize_filename(file, use_color);
    print!(
        "{}{}  ",
        colored_name,
        if file.is_dir { "/" } else { "" }
    );
}

fn should_show_file(name: &str, show_all: bool) -> bool {
    show_all || !name.starts_with('.')
}

fn list_directory(path: &Path, opt: &Opt) -> Result<(), LsError> {
    let mut entries = Vec::new();
    let use_color = should_use_color(&opt.color);

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_info = FileInfo::new(entry)?;
            
            if should_show_file(&file_info.name, opt.all) {
                entries.push(file_info);
            }
        }
    } else {
        // Single file
        let file_info = FileInfo::from_path(path)?;
        entries.push(file_info);
    }

    // Sort entries
    if opt.sort_time {
        entries.sort_by_key(|f| f.metadata.modified().unwrap_or(std::time::UNIX_EPOCH));
    } else {
        entries.sort_by(|a, b| a.name.cmp(&b.name));
    }

    if opt.reverse {
        entries.reverse();
    }

    // Print entries
    if opt.long {
        for file in &entries {
            print_long_format(file, opt.human_readable, use_color)?;
        }
    } else {
        for file in &entries {
            print_short_format(file, use_color);
        }
        if !entries.is_empty() {
            println!(); // New line after short format
        }
    }

    Ok(())
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
