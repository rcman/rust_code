use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use egui::{Align2, Context as EguiContext, Response, Ui, Vec2};
use egui_extras::{Column, TableBuilder};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use sysinfo::{System, SystemExt};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct Package {
    name: String,
    version: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct Snapshot {
    id: i64,
    name: String,
    timestamp: DateTime<Utc>,
    packages: Vec<Package>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct Host {
    id: i64,
    name: String,
    os_type: OsType,
    snapshots: Vec<Snapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
enum OsType {
    Fedora,
    Ubuntu,
}

impl OsType {
    fn from_system() -> Result<Self> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let os_release = std::fs::read_to_string("/etc/os-release")
            .context("Failed to read /etc/os-release")?;

        let id_like: String = os_release
            .lines()
            .find(|line| line.starts_with("ID_LIKE="))
            .and_then(|line| line.split('=').nth(1))
            .unwrap_or("")
            .replace('"', "")
            .to_lowercase();

        let id: String = os_release
            .lines()
            .find(|line| line.starts_with("ID="))
            .and_then(|line| line.split('=').nth(1))
            .unwrap_or("")
            .replace('"', "")
            .to_lowercase();

        if id == "fedora" || id_like.contains("fedora") {
            Ok(Self::Fedora)
        } else if id == "ubuntu" || id_like.contains("debian") {
            Ok(Self::Ubuntu)
        } else {
            Err(anyhow::anyhow!("Unsupported OS"))
        }
    }

    fn list_installed_packages(&self) -> Result<Vec<Package>> {
        let output = std::process::Command::new(if *self == OsType::Fedora { "dnf" } else { "apt" })
            .args(if *self == OsType::Fedora {
                vec!["list", "installed", "--quiet"]
            } else {
                vec!["list", "--installed"]
            })
            .output()
            .context("Failed to run package list command")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut packages = Vec::new();
        for line in stdout.lines() {
            if *self == OsType::Fedora {
                if let Some(name_version) = line.split_whitespace().nth(0) {
                    if let Some((name, version)) = name_version.split_once('.') {
                        packages.push(Package {
                            name: name.to_string(),
                            version: version.to_string(),
                        });
                    }
                }
            } else {
                // Ubuntu apt list format: name/version arch [installed]
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 && parts[3] == "[installed]" {
                    if let Some((name, version)) = parts[0].split_once('/') {
                        packages.push(Package {
                            name: name.to_string(),
                            version: version.to_string(),
                        });
                    }
                }
            }
        }
        Ok(packages)
    }

    fn restore_to_snapshot(&self, snapshot_packages: &[Package]) -> Result<()> {
        let current_packages = self.list_installed_packages()?;

        let current_map: HashMap<String, String> = current_packages
            .into_iter()
            .map(|p| (p.name, p.version))
            .collect();

        let target_map: HashMap<String, String> = snapshot_packages
            .iter()
            .map(|p| (p.name.clone(), p.version.clone()))
            .collect();

        let mut to_install = Vec::new();
        let mut to_remove = Vec::new();

        for (name, version) in &target_map {
            if !current_map.contains_key(name) || current_map[name] != *version {
                to_install.push((name.clone(), version.clone()));
            }
        }

        for name in current_map.keys() {
            if !target_map.contains_key(name) {
                to_remove.push(name.clone());
            }
        }

        // Remove first
        for name in &to_remove {
            let cmd = if *self == OsType::Fedora {
                vec!["remove", "-y", name]
            } else {
                vec!["remove", "-y", name]
            };
            std::process::Command::new(if *self == OsType::Fedora { "dnf" } else { "apt-get" })
                .args(&cmd)
                .status()
                .context(format!("Failed to remove {}", name))?;
        }

        // Install
        for (name, version) in &to_install {
            let cmd = if *self == OsType::Fedora {
                vec!["install", "-y", &format!("{}-{}", name, version)]
            } else {
                vec!["install", "-y", &format!("{}={}", name, version)]
            };
            std::process::Command::new(if *self == OsType::Fedora { "dnf" } else { "apt-get" })
                .args(&cmd)
                .status()
                .context(format!("Failed to install {}-{}", name, version))?;
        }

        Ok(())
    }
}

struct PackageManagerApp {
    conn: Connection,
    host: Option<Host>,
    selected_snapshot: Option<usize>,
    compare_snapshot: Option<usize>,
    show_diff: bool,
    context_menu: Option<ContextMenu>,
    status_message: String,
}

#[derive(Debug)]
enum ContextMenu {
    Restore(usize),
}

impl Default for PackageManagerApp {
    fn default() -> Self {
        // Use unwrap_or_else to handle the Result from creating the database
        let app = Self::new().unwrap_or_else(|e| {
            eprintln!("Failed to initialize app: {}", e);
            std::process::exit(1);
        });
        app
    }
}

impl PackageManagerApp {
    fn new() -> Result<Self> {
        let conn = Connection::open("packages.db")?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS hosts (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                os_type TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY,
                host_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (host_id) REFERENCES hosts (id)
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS packages (
                id INTEGER PRIMARY KEY,
                snapshot_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                FOREIGN KEY (snapshot_id) REFERENCES snapshots (id)
            )",
            [],
        )?;

        let os_type = OsType::from_system().unwrap_or(OsType::Ubuntu);
        let host_name = sysinfo::System::host_name().unwrap_or_else(|| "Local Host".to_string());
        
        let host_id = if let Some(row) = conn
            .query_row(
                "SELECT id FROM hosts WHERE name = ?1",
                [host_name.clone()],
                |row| row.get(0),
            )
            .optional()?
        {
            row
        } else {
            conn.execute(
                "INSERT INTO hosts (name, os_type) VALUES (?1, ?2)",
                params![host_name.clone(), format!("{:?}", os_type)],
            )?;
            conn.last_insert_rowid()
        };

        let mut host = Host {
            id: host_id,
            name: host_name,
            os_type,
            snapshots: Vec::new(),
        };

        let mut app = Self {
            conn,
            host: Some(host),
            selected_snapshot: None,
            compare_snapshot: None,
            show_diff: false,
            context_menu: None,
            status_message: String::new(),
        };

        // Load existing snapshots
        if let Some(ref mut host) = app.host {
            app.load_snapshots(host)?;
        }

        Ok(app)
    }
}

impl eframe::App for PackageManagerApp {
    fn update(&mut self, ctx: &EguiContext, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(host) = &mut self.host {
                ui.heading(&host.name);

                if ui.button("Create Snapshot").clicked() {
                    match host.os_type.list_installed_packages() {
                        Ok(packages) => {
                            let timestamp = Utc::now();
                            let name = format!("Snapshot at {}", timestamp.format("%Y-%m-%d %H:%M:%S"));
                            match self.conn.execute(
                                "INSERT INTO snapshots (host_id, name, timestamp) VALUES (?1, ?2, ?3)",
                                params![host.id, name.clone(), timestamp.to_rfc3339()],
                            ) {
                                Ok(_) => {
                                    let snapshot_id = self.conn.last_insert_rowid();
                                    for pkg in &packages {
                                        if let Err(e) = self.conn.execute(
                                            "INSERT INTO packages (snapshot_id, name, version) VALUES (?1, ?2, ?3)",
                                            params![snapshot_id, pkg.name, pkg.version],
                                        ) {
                                            self.status_message = format!("Error inserting package: {}", e);
                                            break;
                                        }
                                    }
                                    // Reload snapshots
                                    if let Err(e) = self.load_snapshots(host) {
                                        self.status_message = format!("Error loading snapshots: {}", e);
                                    } else {
                                        self.status_message = "Snapshot created successfully!".to_string();
                                    }
                                }
                                Err(e) => {
                                    self.status_message = format!("Error creating snapshot: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            self.status_message = format!("Error listing packages: {}", e);
                        }
                    }
                }

                if !self.status_message.is_empty() {
                    ui.label(&self.status_message);
                    if ui.button("Clear").clicked() {
                        self.status_message.clear();
                    }
                }

                TableBuilder::new(ui)
                    .striped(true)
                    .column(Column::auto())
                    .column(Column::auto())
                    .header(20.0, |mut header| {
                        header.col(|ui| { ui.strong("Snapshot Name"); });
                        header.col(|ui| { ui.strong("Timestamp"); });
                    })
                    .body(|mut body| {
                        for (i, snapshot) in host.snapshots.iter().enumerate() {
                            body.row(18.0, |mut row| {
                                let response = row.col(|ui| {
                                    let resp = ui.selectable_label(self.selected_snapshot == Some(i), &snapshot.name);
                                    if resp.clicked() {
                                        self.selected_snapshot = Some(i);
                                        self.show_diff = false;
                                    }
                                    resp
                                });
                                row.col(|ui| {
                                    ui.label(snapshot.timestamp.format("%Y-%m-%d %H:%M").to_string());
                                });

                                if response.response.right_clicked() {
                                    self.context_menu = Some(ContextMenu::Restore(i));
                                }
                            });
                        }
                    });

                if let Some(i) = self.selected_snapshot {
                    if i < host.snapshots.len() {
                        ui.separator();
                        ui.heading(&host.snapshots[i].name);
                        if ui.button("Compare with another snapshot").clicked() {
                            self.show_diff = true;
                        }
                        if self.show_diff {
                            ui.horizontal(|ui| {
                                ui.label("Compare to:");
                                egui::ComboBox::from_label("")
                                    .selected_text(if let Some(j) = self.compare_snapshot { 
                                        if j < host.snapshots.len() {
                                            host.snapshots[j].name.clone()
                                        } else {
                                            "Invalid selection".to_string()
                                        }
                                    } else { 
                                        "Select a snapshot".to_string() 
                                    })
                                    .show_ui(ui, |ui| {
                                        for (j, snap) in host.snapshots.iter().enumerate() {
                                            if j != i {
                                                if ui.selectable_label(self.compare_snapshot == Some(j), &snap.name).clicked() {
                                                    self.compare_snapshot = Some(j);
                                                }
                                            }
                                        }
                                    });
                            });
                            if let Some(j) = self.compare_snapshot {
                                if j < host.snapshots.len() {
                                    if let Some(diff) = self.compute_diff(&host.snapshots[i], &host.snapshots[j]) {
                                        self.show_diff_table(ui, &diff);
                                    }
                                }
                            }
                        }
                    }
                }

                // Handle context menu
                if let Some(ContextMenu::Restore(i)) = self.context_menu.take() {
                    egui::Window::new("Restore Confirmation")
                        .collapsible(false)
                        .show(ctx, |ui| {
                            ui.label("Are you sure you want to restore to this snapshot?");
                            ui.label("This will modify your system packages!");
                            ui.horizontal(|ui| {
                                if ui.button("Yes, Restore").clicked() {
                                    if let Some(host) = &self.host {
                                        if i < host.snapshots.len() {
                                            match host.os_type.restore_to_snapshot(&host.snapshots[i].packages) {
                                                Ok(_) => {
                                                    self.status_message = "Restore completed successfully!".to_string();
                                                }
                                                Err(e) => {
                                                    self.status_message = format!("Restore failed: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    self.context_menu = None;
                                }
                                if ui.button("Cancel").clicked() {
                                    self.context_menu = None;
                                }
                            });
                        });
                }
            }
        });
    }
}

impl PackageManagerApp {
    fn load_snapshots(&mut self, host: &mut Host) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, timestamp FROM snapshots WHERE host_id = ?1 ORDER BY timestamp DESC"
        )?;
        let snapshot_iter = stmt.query_map([host.id], |row| {
            Ok(Snapshot {
                id: row.get(0)?,
                name: row.get(1)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .map_err(|e| rusqlite::Error::InvalidColumnType(2, "timestamp".to_string(), rusqlite::types::Type::Text))?
                    .with_timezone(&Utc),
                packages: Vec::new(),
            })
        })?;

        host.snapshots.clear();
        for snap in snapshot_iter {
            let mut snapshot = snap?;
            let mut pkg_stmt = self.conn.prepare(
                "SELECT name, version FROM packages WHERE snapshot_id = ?1"
            )?;
            let pkg_iter = pkg_stmt.query_map([snapshot.id], |row| {
                Ok(Package {
                    name: row.get(0)?,
                    version: row.get(1)?,
                })
            })?;
            snapshot.packages = pkg_iter.collect::<Result<Vec<_>, _>>()?;
            host.snapshots.push(snapshot);
        }
        Ok(())
    }

    fn compute_diff(&self, snap1: &Snapshot, snap2: &Snapshot) -> Option<Diff> {
        let map1: HashMap<String, String> = snap1.packages.iter().map(|p| (p.name.clone(), p.version.clone())).collect();
        let map2: HashMap<String, String> = snap2.packages.iter().map(|p| (p.name.clone(), p.version.clone())).collect();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();

        for (name, ver) in &map2 {
            match map1.get(name) {
                Some(old_ver) if old_ver != ver => changed.push((name.clone(), old_ver.clone(), ver.clone())),
                Some(_) => {},
                None => added.push((name.clone(), ver.clone())),
            }
        }

        for name in map1.keys() {
            if !map2.contains_key(name) {
                removed.push((name.clone(), map1[name].clone()));
            }
        }

        Some(Diff { added, removed, changed })
    }

    fn show_diff_table(&self, ui: &mut Ui, diff: &Diff) {
        ui.heading("Differences");

        if !diff.added.is_empty() {
            ui.collapsing("Added", |ui| {
                TableBuilder::new(ui)
                    .column(Column::auto())
                    .column(Column::auto())
                    .header(20.0, |mut header| {
                        header.col(|ui| { ui.strong("Package"); });
                        header.col(|ui| { ui.strong("Version"); });
                    })
                    .body(|mut body| {
                        for (name, ver) in &diff.added {
                            body.row(18.0, |mut row| {
                                row.col(|ui| { ui.label(name); });
                                row.col(|ui| { ui.label(ver); });
                            });
                        }
                    });
            });
        }

        if !diff.removed.is_empty() {
            ui.collapsing("Removed", |ui| {
                TableBuilder::new(ui)
                    .column(Column::auto())
                    .column(Column::auto())
                    .header(20.0, |mut header| {
                        header.col(|ui| { ui.strong("Package"); });
                        header.col(|ui| { ui.strong("Version"); });
                    })
                    .body(|mut body| {
                        for (name, ver) in &diff.removed {
                            body.row(18.0, |mut row| {
                                row.col(|ui| { ui.label(name); });
                                row.col(|ui| { ui.label(ver); });
                            });
                        }
                    });
            });
        }

        if !diff.changed.is_empty() {
            ui.collapsing("Changed", |ui| {
                TableBuilder::new(ui)
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::auto())
                    .header(20.0, |mut header| {
                        header.col(|ui| { ui.strong("Package"); });
                        header.col(|ui| { ui.strong("Old Version"); });
                        header.col(|ui| { ui.strong("New Version"); });
                    })
                    .body(|mut body| {
                        for (name, old, new) in &diff.changed {
                            body.row(18.0, |mut row| {
                                row.col(|ui| { ui.label(name); });
                                row.col(|ui| { ui.label(old); });
                                row.col(|ui| { ui.label(new); });
                            });
                        }
                    });
            });
        }
    }
}

#[derive(Debug)]
struct Diff {
    added: Vec<(String, String)>,
    removed: Vec<(String, String)>,
    changed: Vec<(String, String, String)>,
}

fn main() -> Result<()> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(Vec2::new(800.0, 600.0)),
        ..Default::default()
    };
    eframe::run_native(
        "Package Snapshot Manager",
        options,
        Box::new(|_cc| Box::new(PackageManagerApp::default())),
    ).map_err(|e| anyhow::anyhow!("Failed to run application: {}", e))?;
    Ok(())
}
