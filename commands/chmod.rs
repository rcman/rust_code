use std::env;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process;
use std::collections::VecDeque;

// Permission bit constants
const S_IRWXU: u32 = 0o700; // User read, write, execute
const S_IRUSR: u32 = 0o400; // User read
const S_IWUSR: u32 = 0o200; // User write  
const S_IXUSR: u32 = 0o100; // User execute

const S_IRWXG: u32 = 0o070; // Group read, write, execute
const S_IRGRP: u32 = 0o040; // Group read
const S_IWGRP: u32 = 0o020; // Group write
const S_IXGRP: u32 = 0o010; // Group execute

const S_IRWXO: u32 = 0o007; // Other read, write, execute
const S_IROTH: u32 = 0o004; // Other read
const S_IWOTH: u32 = 0o002; // Other write
const S_IXOTH: u32 = 0o001; // Other execute

const S_ISUID: u32 = 0o4000; // Set user ID
const S_ISGID: u32 = 0o2000; // Set group ID
const S_ISVTX: u32 = 0o1000; // Sticky bit

const CHMOD_MODE_BITS: u32 = S_ISUID | S_ISGID | S_ISVTX | S_IRWXU | S_IRWXG | S_IRWXO;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Verbosity {
    Off,
    ChangesOnly,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ChangeStatus {
    NoStat,
    Failed,
    NotApplied,
    NoChangeRequested,
    Succeeded,
}

#[derive(Debug)]
struct ChangeInfo {
    status: ChangeStatus,
    old_mode: u32,
    new_mode: u32,
}

#[derive(Debug, Clone, Copy)]
enum ModeOperation {
    Set,    // =
    Add,    // +
    Remove, // -
}

#[derive(Debug, Clone, Copy)]
enum ModeTarget {
    User,   // u
    Group,  // g
    Other,  // o
    All,    // a
}

#[derive(Debug, Clone)]
struct ModeChange {
    targets: Vec<ModeTarget>,
    operation: ModeOperation,
    permissions: u32,
    copy_from: Option<ModeTarget>,
}

#[derive(Debug)]
struct Config {
    verbosity: Verbosity,
    recursive: bool,
    dereference: Option<bool>,
    force_silent: bool,
    diagnose_surprises: bool,
    preserve_root: bool,
    reference_file: Option<PathBuf>,
    mode_changes: Vec<ModeChange>,
    files: Vec<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            verbosity: Verbosity::Off,
            recursive: false,
            dereference: None,
            force_silent: false,
            diagnose_surprises: false,
            preserve_root: false,
            reference_file: None,
            mode_changes: Vec::new(),
            files: Vec::new(),
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    let config = match parse_args(&args) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("chmod: {}", e);
            process::exit(1);
        }
    };

    let success = process_files(&config);
    
    process::exit(if success { 0 } else { 1 });
}

fn parse_args(args: &[String]) -> Result<Config, String> {
    if args.len() < 2 {
        return Err("missing operand".to_string());
    }

    let mut config = Config::default();
    let mut i = 1;
    let mut mode_str = String::new();

    while i < args.len() {
        let arg = &args[i];
        
        if !arg.starts_with('-') {
            // First non-option argument is the mode (unless using --reference)
            if config.reference_file.is_none() && mode_str.is_empty() {
                mode_str = arg.clone();
                i += 1;
                continue;
            }
            // Remaining arguments are files
            config.files.extend(args[i..].iter().map(|s| PathBuf::from(s)));
            break;
        }

        match arg.as_str() {
            "-c" | "--changes" => config.verbosity = Verbosity::ChangesOnly,
            "-f" | "--silent" | "--quiet" => config.force_silent = true,
            "-v" | "--verbose" => config.verbosity = Verbosity::High,
            "-R" | "--recursive" => config.recursive = true,
            "-h" | "--no-dereference" => config.dereference = Some(false),
            "--dereference" => config.dereference = Some(true),
            "--preserve-root" => config.preserve_root = true,
            "--no-preserve-root" => config.preserve_root = false,
            "--help" => {
                print_help();
                process::exit(0);
            }
            "--version" => {
                println!("chmod (Rust implementation) 1.0.0");
                process::exit(0);
            }
            arg if arg.starts_with("--reference=") => {
                let ref_file = &arg[12..];
                config.reference_file = Some(PathBuf::from(ref_file));
            }
            _ => {
                // Check for short options that might be mode specifications
                if arg.len() > 1 && arg.chars().nth(1).unwrap().is_ascii_digit() {
                    // Looks like an octal mode (e.g., -755)
                    mode_str = arg[1..].to_string();
                    config.diagnose_surprises = true;
                } else {
                    return Err(format!("invalid option: {}", arg));
                }
            }
        }
        i += 1;
    }

    if config.files.is_empty() {
        return Err("missing file operand".to_string());
    }

    // Parse mode changes
    if let Some(ref_file) = &config.reference_file {
        config.mode_changes = vec![create_reference_mode_change(ref_file)?];
    } else if !mode_str.is_empty() {
        config.mode_changes = parse_mode(&mode_str)?;
    } else {
        return Err("missing mode specification".to_string());
    }

    Ok(config)
}

fn parse_mode(mode_str: &str) -> Result<Vec<ModeChange>, String> {
    let mut changes = Vec::new();
    
    // Handle octal mode (e.g., "755", "0644")
    if mode_str.chars().all(|c| c.is_ascii_digit()) {
        let mode_val = u32::from_str_radix(mode_str, 8)
            .map_err(|_| format!("invalid octal mode: {}", mode_str))?;
        
        changes.push(ModeChange {
            targets: vec![ModeTarget::All],
            operation: ModeOperation::Set,
            permissions: mode_val,
            copy_from: None,
        });
        return Ok(changes);
    }

    // Parse symbolic mode (e.g., "u+x", "go-w", "a=r")
    let parts: Vec<&str> = mode_str.split(',').collect();
    for part in parts {
        changes.push(parse_symbolic_mode(part)?);
    }

    Ok(changes)
}

fn parse_symbolic_mode(mode_part: &str) -> Result<ModeChange, String> {
    let mut targets = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = mode_part.chars().collect();

    // Parse targets (u, g, o, a)
    while i < chars.len() {
        match chars[i] {
            'u' => targets.push(ModeTarget::User),
            'g' => targets.push(ModeTarget::Group),
            'o' => targets.push(ModeTarget::Other),
            'a' => targets.push(ModeTarget::All),
            '+' | '-' | '=' => break,
            _ => return Err(format!("invalid mode character: {}", chars[i])),
        }
        i += 1;
    }

    if targets.is_empty() {
        targets.push(ModeTarget::All);
    }

    // Parse operation
    if i >= chars.len() {
        return Err("missing operation in mode".to_string());
    }

    let operation = match chars[i] {
        '+' => ModeOperation::Add,
        '-' => ModeOperation::Remove,
        '=' => ModeOperation::Set,
        _ => return Err(format!("invalid operation: {}", chars[i])),
    };
    i += 1;

    // Parse permissions
    let mut permissions = 0;
    let mut copy_from = None;

    while i < chars.len() {
        match chars[i] {
            'r' => permissions |= S_IRUSR | S_IRGRP | S_IROTH,
            'w' => permissions |= S_IWUSR | S_IWGRP | S_IWOTH,
            'x' => permissions |= S_IXUSR | S_IXGRP | S_IXOTH,
            'X' => permissions |= S_IXUSR | S_IXGRP | S_IXOTH, // Execute only if already executable
            's' => permissions |= S_ISUID | S_ISGID,
            't' => permissions |= S_ISVTX,
            'u' => copy_from = Some(ModeTarget::User),
            'g' => copy_from = Some(ModeTarget::Group),
            'o' => copy_from = Some(ModeTarget::Other),
            _ => return Err(format!("invalid permission character: {}", chars[i])),
        }
        i += 1;
    }

    Ok(ModeChange {
        targets,
        operation,
        permissions,
        copy_from,
    })
}

fn create_reference_mode_change(ref_file: &Path) -> Result<ModeChange, String> {
    let metadata = fs::metadata(ref_file)
        .map_err(|e| format!("failed to get attributes of {}: {}", ref_file.display(), e))?;
    
    let mode = metadata.permissions().mode();
    
    Ok(ModeChange {
        targets: vec![ModeTarget::All],
        operation: ModeOperation::Set,
        permissions: mode,
        copy_from: None,
    })
}

fn process_files(config: &Config) -> bool {
    let mut success = true;
    
    for file_path in &config.files {
        if config.recursive {
            success &= process_file_recursive(config, file_path);
        } else {
            success &= process_single_file(config, file_path);
        }
    }
    
    success
}

fn process_file_recursive(config: &Config, root_path: &Path) -> bool {
    let mut success = true;
    let mut queue = VecDeque::new();
    queue.push_back(root_path.to_path_buf());
    
    while let Some(current_path) = queue.pop_front() {
        // Check for root directory protection
        if config.preserve_root && is_root_directory(&current_path) {
            if !config.force_silent {
                eprintln!("chmod: it is dangerous to operate recursively on '/'");
            }
            continue;
        }
        
        success &= process_single_file(config, &current_path);
        
        if current_path.is_dir() {
            match fs::read_dir(&current_path) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => queue.push_back(entry.path()),
                            Err(e) => {
                                if !config.force_silent {
                                    eprintln!("chmod: cannot read directory {}: {}", 
                                            current_path.display(), e);
                                }
                                success = false;
                            }
                        }
                    }
                }
                Err(e) => {
                    if !config.force_silent {
                        eprintln!("chmod: cannot read directory {}: {}", 
                                current_path.display(), e);
                    }
                    success = false;
                }
            }
        }
    }
    
    success
}

fn process_single_file(config: &Config, file_path: &Path) -> bool {
    let metadata = match fs::metadata(file_path) {
        Ok(metadata) => metadata,
        Err(e) => {
            if !config.force_silent {
                eprintln!("chmod: cannot access {}: {}", file_path.display(), e);
            }
            return false;
        }
    };
    
    let old_mode = metadata.permissions().mode();
    let new_mode = apply_mode_changes(&config.mode_changes, old_mode, metadata.is_dir());
    
    let mut change_info = ChangeInfo {
        status: ChangeStatus::NotApplied,
        old_mode,
        new_mode,
    };
    
    if old_mode != new_mode {
        let mut permissions = fs::Permissions::from_mode(new_mode);
        
        match fs::set_permissions(file_path, permissions) {
            Ok(_) => change_info.status = ChangeStatus::Succeeded,
            Err(e) => {
                if !config.force_silent {
                    eprintln!("chmod: changing permissions of {}: {}", 
                            file_path.display(), e);
                }
                change_info.status = ChangeStatus::Failed;
            }
        }
    } else {
        change_info.status = ChangeStatus::NoChangeRequested;
    }
    
    // Handle verbosity
    match config.verbosity {
        Verbosity::High => describe_change(file_path, &change_info),
        Verbosity::ChangesOnly if change_info.status == ChangeStatus::Succeeded => {
            describe_change(file_path, &change_info)
        }
        _ => {}
    }
    
    // Diagnose surprises if requested
    if config.diagnose_surprises && change_info.status >= ChangeStatus::NoChangeRequested {
        diagnose_mode_surprises(file_path, &change_info, &config.mode_changes, metadata.is_dir());
    }
    
    change_info.status != ChangeStatus::Failed
}

fn apply_mode_changes(changes: &[ModeChange], current_mode: u32, is_dir: bool) -> u32 {
    let mut new_mode = current_mode;
    
    for change in changes {
        new_mode = apply_single_mode_change(change, new_mode, current_mode, is_dir);
    }
    
    new_mode
}

fn apply_single_mode_change(change: &ModeChange, current_mode: u32, original_mode: u32, is_dir: bool) -> u32 {
    let mut new_mode = current_mode;
    
    for &target in &change.targets {
        let target_mask = get_target_mask(target);
        let permissions = if let Some(copy_target) = change.copy_from {
            extract_permissions_for_target(original_mode, copy_target, target)
        } else {
            adjust_permissions_for_target(change.permissions, target, is_dir)
        };
        
        match change.operation {
            ModeOperation::Set => {
                new_mode = (new_mode & !target_mask) | (permissions & target_mask);
            }
            ModeOperation::Add => {
                new_mode |= permissions & target_mask;
            }
            ModeOperation::Remove => {
                new_mode &= !(permissions & target_mask);
            }
        }
    }
    
    new_mode
}

fn get_target_mask(target: ModeTarget) -> u32 {
    match target {
        ModeTarget::User => S_IRWXU | S_ISUID,
        ModeTarget::Group => S_IRWXG | S_ISGID,
        ModeTarget::Other => S_IRWXO,
        ModeTarget::All => CHMOD_MODE_BITS,
    }
}

fn extract_permissions_for_target(mode: u32, source_target: ModeTarget, dest_target: ModeTarget) -> u32 {
    let source_perms = match source_target {
        ModeTarget::User => (mode & S_IRWXU) >> 6,
        ModeTarget::Group => (mode & S_IRWXG) >> 3,
        ModeTarget::Other => mode & S_IRWXO,
        ModeTarget::All => mode,
    };
    
    match dest_target {
        ModeTarget::User => source_perms << 6,
        ModeTarget::Group => source_perms << 3,
        ModeTarget::Other => source_perms,
        ModeTarget::All => source_perms,
    }
}

fn adjust_permissions_for_target(permissions: u32, target: ModeTarget, _is_dir: bool) -> u32 {
    match target {
        ModeTarget::User => permissions & (S_IRWXU | S_ISUID),
        ModeTarget::Group => permissions & (S_IRWXG | S_ISGID),
        ModeTarget::Other => permissions & S_IRWXO,
        ModeTarget::All => permissions,
    }
}

fn describe_change(file_path: &Path, change: &ChangeInfo) {
    match change.status {
        ChangeStatus::Succeeded => {
            println!("mode of {} changed from {:04o} to {:04o}",
                    file_path.display(),
                    change.old_mode & CHMOD_MODE_BITS,
                    change.new_mode & CHMOD_MODE_BITS);
        }
        ChangeStatus::Failed => {
            println!("failed to change mode of {} from {:04o} to {:04o}",
                    file_path.display(),
                    change.old_mode & CHMOD_MODE_BITS,
                    change.new_mode & CHMOD_MODE_BITS);
        }
        ChangeStatus::NoChangeRequested => {
            println!("mode of {} retained as {:04o}",
                    file_path.display(),
                    change.new_mode & CHMOD_MODE_BITS);
        }
        _ => {}
    }
}

fn diagnose_mode_surprises(_file_path: &Path, _change: &ChangeInfo, _changes: &[ModeChange], _is_dir: bool) {
    // Implementation for diagnosing surprising mode changes
    // This would warn about umask-related surprises in the original chmod
}

fn is_root_directory(path: &Path) -> bool {
    path == Path::new("/")
}

fn print_help() {
    println!("Usage: chmod [OPTION]... MODE[,MODE]... FILE...");
    println!("  or:  chmod [OPTION]... OCTAL-MODE FILE...");
    println!("  or:  chmod [OPTION]... --reference=RFILE FILE...");
    println!();
    println!("Change the mode of each FILE to MODE.");
    println!("With --reference, change the mode of each FILE to that of RFILE.");
    println!();
    println!("  -c, --changes          like verbose but report only when a change is made");
    println!("  -f, --silent, --quiet  suppress most error messages");
    println!("  -v, --verbose          output a diagnostic for every file processed");
    println!("      --dereference      affect the referent of each symbolic link");
    println!("  -h, --no-dereference   affect symbolic links instead of referenced files");
    println!("      --no-preserve-root do not treat '/' specially (the default)");
    println!("      --preserve-root    fail to operate recursively on '/'");
    println!("      --reference=RFILE  use RFILE's mode instead of MODE values");
    println!("  -R, --recursive        change files and directories recursively");
    println!("      --help             display this help and exit");
    println!("      --version          output version information and exit");
    println!();
    println!("Each MODE is of the form '[ugoa]*([-+=]([rwxXst]*|[ugo]))+'.");
}
