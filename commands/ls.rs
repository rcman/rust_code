use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::collections::HashMap;
use std::cmp::Ordering;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

// Equivalent to the C header constants
#[derive(Debug, Clone, Copy, PartialEq)]
enum LsMode {
    Ls = 1,
    MultiCol = 2,
    LongFormat = 3,
}

// File types similar to the C enum
#[derive(Debug, Clone, Copy, PartialEq)]
enum FileType {
    Unknown,
    Fifo,
    CharDev,
    Directory,
    BlockDev,
    Normal,
    SymbolicLink,
    Socket,
    Whiteout,
    ArgDirectory,
}

impl FileType {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        use std::os::unix::fs::FileTypeExt;
        
        let file_type = metadata.file_type();
        
        if file_type.is_dir() {
            FileType::Directory
        } else if file_type.is_file() {
            FileType::Normal
        } else if file_type.is_symlink() {
            FileType::SymbolicLink
        } else if file_type.is_fifo() {
            FileType::Fifo
        } else if file_type.is_char_device() {
            FileType::CharDev
        } else if file_type.is_block_device() {
            FileType::BlockDev
        } else if file_type.is_socket() {
            FileType::Socket
        } else {
            FileType::Unknown
        }
    }

    fn letter(&self) -> char {
        match self {
            FileType::Unknown => '?',
            FileType::Fifo => 'p',
            FileType::CharDev => 'c',
            FileType::Directory => 'd',
            FileType::BlockDev => 'b',
            FileType::Normal => '-',
            FileType::SymbolicLink => 'l',
            FileType::Socket => 's',
            FileType::Whiteout => 'w',
            FileType::ArgDirectory => 'd',
        }
    }
}

// Display format options
#[derive(Debug, Clone, Copy, PartialEq)]
enum Format {
    LongFormat,
    OnePerLine,
    ManyPerLine,
    Horizontal,
    WithCommas,
}

// Sort options
#[derive(Debug, Clone, Copy, PartialEq)]
enum SortType {
    Name,
    Extension,
    Size,
    Time,
    None,
}

// Time type for sorting/display
#[derive(Debug, Clone, Copy, PartialEq)]
enum TimeType {
    Mtime,
    Ctime,
    Atime,
}

// Configuration structure
#[derive(Debug, Clone)]
struct Config {
    ls_mode: LsMode,
    format: Format,
    sort_type: SortType,
    sort_reverse: bool,
    time_type: TimeType,
    print_owner: bool,
    print_group: bool,
    print_inode: bool,
    print_block_size: bool,
    numeric_ids: bool,
    show_all: bool,
    show_almost_all: bool,
    recursive: bool,
    immediate_dirs: bool,
    long_format: bool,
    dereference: bool,
    show_hidden: bool,
    line_length: usize,
    color: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ls_mode: LsMode::Ls,
            format: Format::ManyPerLine,
            sort_type: SortType::Name,
            sort_reverse: false,
            time_type: TimeType::Mtime,
            print_owner: true,
            print_group: true,
            print_inode: false,
            print_block_size: false,
            numeric_ids: false,
            show_all: false,
            show_almost_all: false,
            recursive: false,
            immediate_dirs: false,
            long_format: false,
            dereference: false,
            show_hidden: false,
            line_length: 80,
            color: false,
        }
    }
}

// File information structure
#[derive(Debug, Clone)]
struct FileInfo {
    name: String,
    path: PathBuf,
    file_type: FileType,
    metadata: Option<fs::Metadata>,
    is_symlink: bool,
    link_target: Option<String>,
}

impl FileInfo {
    fn new(path: PathBuf, config: &Config) -> io::Result<Self> {
        let name = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let metadata = if config.dereference {
            fs::metadata(&path).ok()
        } else {
            fs::symlink_metadata(&path).ok()
        };

        let is_symlink = path.is_symlink();
        let file_type = if let Some(ref meta) = metadata {
            FileType::from_metadata(meta)
        } else {
            FileType::Unknown
        };

        let link_target = if is_symlink {
            fs::read_link(&path).ok().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };

        Ok(FileInfo {
            name,
            path,
            file_type,
            metadata,
            is_symlink,
            link_target,
        })
    }

    fn should_show(&self, config: &Config) -> bool {
        if config.show_all {
            return true;
        }

        if self.name.starts_with('.') {
            if config.show_almost_all {
                return self.name != "." && self.name != "..";
            }
            return false;
        }

        true
    }

    fn get_time(&self, time_type: TimeType) -> SystemTime {
        if let Some(ref meta) = self.metadata {
            match time_type {
                TimeType::Mtime => meta.modified().unwrap_or(UNIX_EPOCH),
                TimeType::Ctime => SystemTime::UNIX_EPOCH + 
                    std::time::Duration::from_secs(meta.ctime() as u64),
                TimeType::Atime => meta.accessed().unwrap_or(UNIX_EPOCH),
            }
        } else {
            UNIX_EPOCH
        }
    }
}

// Main LS implementation
struct Ls {
    config: Config,
    files: Vec<FileInfo>,
}

impl Ls {
    fn new() -> Self {
        Ls {
            config: Config::default(),
            files: Vec::new(),
        }
    }

    fn parse_args(&mut self, args: Vec<String>) -> Result<Vec<PathBuf>, String> {
        let mut paths = Vec::new();
        let mut i = 1;

        while i < args.len() {
            let arg = &args[i];
            
            if !arg.starts_with('-') {
                paths.push(PathBuf::from(arg));
                i += 1;
                continue;
            }

            match arg.as_str() {
                "-l" => {
                    self.config.format = Format::LongFormat;
                    self.config.long_format = true;
                }
                "-a" => self.config.show_all = true,
                "-A" => self.config.show_almost_all = true,
                "-1" => self.config.format = Format::OnePerLine,
                "-C" => self.config.format = Format::ManyPerLine,
                "-x" => self.config.format = Format::Horizontal,
                "-m" => self.config.format = Format::WithCommas,
                "-r" => self.config.sort_reverse = true,
                "-t" => self.config.sort_type = SortType::Time,
                "-S" => self.config.sort_type = SortType::Size,
                "-X" => self.config.sort_type = SortType::Extension,
                "-U" => self.config.sort_type = SortType::None,
                "-R" => self.config.recursive = true,
                "-d" => self.config.immediate_dirs = true,
                "-i" => self.config.print_inode = true,
                "-s" => self.config.print_block_size = true,
                "-n" => self.config.numeric_ids = true,
                "-g" => {
                    self.config.format = Format::LongFormat;
                    self.config.print_owner = false;
                }
                "-o" => {
                    self.config.format = Format::LongFormat;
                    self.config.print_group = false;
                }
                "-L" => self.config.dereference = true,
                "-u" => self.config.time_type = TimeType::Atime,
                "-c" => self.config.time_type = TimeType::Ctime,
                "--color" => self.config.color = true,
                "--help" => {
                    self.print_help();
                    std::process::exit(0);
                }
                "--version" => {
                    println!("ls (Rust implementation) 1.0.0");
                    std::process::exit(0);
                }
                _ => return Err(format!("Unknown option: {}", arg)),
            }
            i += 1;
        }

        if paths.is_empty() {
            paths.push(PathBuf::from("."));
        }

        Ok(paths)
    }

    fn print_help(&self) {
        println!("Usage: ls [OPTION]... [FILE]...");
        println!("List information about the FILEs (the current directory by default).");
        println!();
        println!("Options:");
        println!("  -a, --all                  do not ignore entries starting with .");
        println!("  -A, --almost-all           do not list implied . and ..");
        println!("  -C                         list entries by columns");
        println!("  -l                         use a long listing format");
        println!("  -1                         list one file per line");
        println!("  -r, --reverse              reverse order while sorting");
        println!("  -R, --recursive            list subdirectories recursively");
        println!("  -S                         sort by file size, largest first");
        println!("  -t                         sort by modification time, newest first");
        println!("  -u                         sort by access time");
        println!("  -c                         sort by status change time");
        println!("  -X                         sort alphabetically by entry extension");
        println!("  -U                         do not sort; list entries in directory order");
        println!("  -d, --directory            list directories themselves, not their contents");
        println!("  -i, --inode                print the index number of each file");
        println!("  -s, --size                 print the allocated size of each file");
        println!("  -n, --numeric-uid-gid      list numeric user and group IDs");
        println!("  -g                         like -l, but do not list owner");
        println!("  -o                         like -l, but do not list group information");
        println!("  -L, --dereference          show information for file references");
        println!("      --color                colorize the output");
        println!("      --help                 display this help and exit");
        println!("      --version              output version information and exit");
    }

    fn collect_files(&mut self, paths: &[PathBuf]) -> io::Result<()> {
        for path in paths {
            if path.is_dir() && !self.config.immediate_dirs {
                self.collect_directory_files(path)?;
            } else {
                let file_info = FileInfo::new(path.clone(), &self.config)?;
                if file_info.should_show(&self.config) {
                    self.files.push(file_info);
                }
            }
        }
        Ok(())
    }

    fn collect_directory_files(&mut self, dir_path: &Path) -> io::Result<()> {
        let entries = fs::read_dir(dir_path)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_info = FileInfo::new(path, &self.config)?;
            
            if file_info.should_show(&self.config) {
                self.files.push(file_info);
            }
        }
        
        Ok(())
    }

    fn sort_files(&mut self) {
        if self.config.sort_type == SortType::None {
            return;
        }

        self.files.sort_by(|a, b| {
            let cmp = match self.config.sort_type {
                SortType::Name => a.name.cmp(&b.name),
                SortType::Extension => {
                    let ext_a = Path::new(&a.name).extension()
                        .unwrap_or_default().to_string_lossy();
                    let ext_b = Path::new(&b.name).extension()
                        .unwrap_or_default().to_string_lossy();
                    ext_a.cmp(&ext_b).then_with(|| a.name.cmp(&b.name))
                }
                SortType::Size => {
                    let size_a = a.metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                    let size_b = b.metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                    size_b.cmp(&size_a).then_with(|| a.name.cmp(&b.name))
                }
                SortType::Time => {
                    let time_a = a.get_time(self.config.time_type);
                    let time_b = b.get_time(self.config.time_type);
                    time_b.cmp(&time_a).then_with(|| a.name.cmp(&b.name))
                }
                SortType::None => Ordering::Equal,
            };

            if self.config.sort_reverse {
                cmp.reverse()
            } else {
                cmp
            }
        });
    }

    fn format_permissions(mode: u32) -> String {
        let mut perms = String::new();
        
        // User permissions
        perms.push(if mode & 0o400 != 0 { 'r' } else { '-' });
        perms.push(if mode & 0o200 != 0 { 'w' } else { '-' });
        perms.push(if mode & 0o100 != 0 { 'x' } else { '-' });
        
        // Group permissions
        perms.push(if mode & 0o040 != 0 { 'r' } else { '-' });
        perms.push(if mode & 0o020 != 0 { 'w' } else { '-' });
        perms.push(if mode & 0o010 != 0 { 'x' } else { '-' });
        
        // Other permissions
        perms.push(if mode & 0o004 != 0 { 'r' } else { '-' });
        perms.push(if mode & 0o002 != 0 { 'w' } else { '-' });
        perms.push(if mode & 0o001 != 0 { 'x' } else { '-' });
        
        perms
    }

    fn format_size(size: u64) -> String {
        if size < 1024 {
            format!("{}", size)
        } else if size < 1024 * 1024 {
            format!("{:.1}K", size as f64 / 1024.0)
        } else if size < 1024 * 1024 * 1024 {
            format!("{:.1}M", size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1}G", size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    fn format_time(time: SystemTime) -> String {
        match time.duration_since(UNIX_EPOCH) {
            Ok(duration) => {
                // Simple time formatting - in a real implementation you'd want proper date formatting
                format!("{}", duration.as_secs())
            }
            Err(_) => "?".to_string(),
        }
    }

    fn print_long_format(&self, file: &FileInfo) {
        let metadata = match &file.metadata {
            Some(meta) => meta,
            None => {
                println!("?????????? ? ? ? ? ? {}", file.name);
                return;
            }
        };

        // File type and permissions
        let mut mode_str = String::new();
        mode_str.push(file.file_type.letter());
        mode_str.push_str(&Self::format_permissions(metadata.permissions().mode()));

        // Number of links
        let nlink = metadata.nlink();

        // Owner and group
        let uid = metadata.uid();
        let gid = metadata.gid();
        let owner = if self.config.numeric_ids {
            uid.to_string()
        } else {
            // In a real implementation, you'd look up usernames
            uid.to_string()
        };
        let group = if self.config.numeric_ids {
            gid.to_string()
        } else {
            // In a real implementation, you'd look up group names
            gid.to_string()
        };

        // Size
        let size = Self::format_size(metadata.len());

        // Time
        let time = Self::format_time(file.get_time(self.config.time_type));

        // Print the formatted line
        print!("{} {:3} ", mode_str, nlink);
        
        if self.config.print_owner {
            print!("{:8} ", owner);
        }
        
        if self.config.print_group {
            print!("{:8} ", group);
        }
        
        print!("{:8} {:12} {}", size, time, file.name);

        // Handle symlinks
        if file.is_symlink {
            if let Some(ref target) = file.link_target {
                print!(" -> {}", target);
            }
        }
        
        println!();
    }

    fn print_simple_format(&self, file: &FileInfo) {
        if self.config.print_inode {
            if let Some(ref meta) = file.metadata {
                print!("{:8} ", meta.ino());
            } else {
                print!("       ? ");
            }
        }

        if self.config.print_block_size {
            if let Some(ref meta) = file.metadata {
                let blocks = (meta.len() + 1023) / 1024; // Simple block calculation
                print!("{:4} ", blocks);
            } else {
                print!("   ? ");
            }
        }

        print!("{}", file.name);
        
        if file.is_symlink {
            if let Some(ref target) = file.link_target {
                print!(" -> {}", target);
            }
        }
    }

    fn display_files(&self) {
        match self.config.format {
            Format::LongFormat => {
                for file in &self.files {
                    self.print_long_format(file);
                }
            }
            Format::OnePerLine => {
                for file in &self.files {
                    self.print_simple_format(file);
                    println!();
                }
            }
            Format::ManyPerLine | Format::Horizontal => {
                // Simple multi-column output
                for (i, file) in self.files.iter().enumerate() {
                    self.print_simple_format(file);
                    if (i + 1) % 4 == 0 || i == self.files.len() - 1 {
                        println!();
                    } else {
                        print!("  ");
                    }
                }
            }
            Format::WithCommas => {
                for (i, file) in self.files.iter().enumerate() {
                    if i > 0 {
                        print!(", ");
                    }
                    self.print_simple_format(file);
                }
                println!();
            }
        }
    }

    fn run(&mut self, args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        let paths = self.parse_args(args)?;
        self.collect_files(&paths)?;
        self.sort_files();
        self.display_files();
        Ok(())
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut ls = Ls::new();
    
    if let Err(e) = ls.run(args) {
        eprintln!("ls: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_detection() {
        // This would require actual files to test properly
        assert_eq!(FileType::Normal.letter(), '-');
        assert_eq!(FileType::Directory.letter(), 'd');
        assert_eq!(FileType::SymbolicLink.letter(), 'l');
    }

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert_eq!(config.format, Format::ManyPerLine);
        assert_eq!(config.sort_type, SortType::Name);
        assert!(!config.sort_reverse);
    }

    #[test]
    fn test_permission_formatting() {
        assert_eq!(Ls::format_permissions(0o755), "rwxr-xr-x");
        assert_eq!(Ls::format_permissions(0o644), "rw-r--r--");
        assert_eq!(Ls::format_permissions(0o000), "---------");
    }

    #[test]
    fn test_size_formatting() {
        assert_eq!(Ls::format_size(512), "512");
        assert_eq!(Ls::format_size(1536), "1.5K");
        assert_eq!(Ls::format_size(1048576), "1.0M");
    }
}
