// Cargo.toml dependencies needed:
/*
[dependencies]
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
dashmap = "5.0"
eframe = "0.24"
egui = "0.24"
egui_plot = "0.24"
rusqlite = { version = "0.29", features = ["bundled"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sysinfo = "0.29"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
ipnetwork = "0.20"
rand = "0.8"
*/

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use eframe::egui;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{error, info, Level};

// Enhanced dataclass equivalents as structs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThreshold {
    pub metric: String,
    pub warning_level: f64,
    pub critical_level: f64,
    pub duration: u64, // seconds
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub device_ip: String,
    pub metric: String,
    pub level: String, // "warning", "critical", "anomaly"
    pub value: f64,
    pub threshold: f64,
    pub timestamp: DateTime<Utc>,
    pub acknowledged: bool,
    pub resolved: bool,
}

#[derive(Debug, Clone)]
pub struct NetworkDevice {
    pub ip: String,
    pub mac: String,
    pub hostname: String,
    pub os_type: String,
    pub status: String,
    pub monitoring_enabled: bool,
    pub username: String,
    pub password: String,
    pub bash_history: String,
    // Enhanced metrics history (deque for fixed-size window)
    pub metrics_history: DashMap<String, VecDeque<(f64, DateTime<Utc>)>>, // metric -> deque of (value, timestamp)
    pub last_update: Option<DateTime<Utc>>,
    pub services: DashMap<String, f64>, // service -> cpu usage
    pub hardware_info: serde_json::Value,
    pub failed_logins: Vec<DateTime<Utc>>,
}

impl Default for NetworkDevice {
    fn default() -> Self {
        Self {
            ip: String::new(),
            mac: String::new(),
            hostname: String::new(),
            os_type: String::new(),
            status: String::new(),
            monitoring_enabled: false,
            username: String::new(),
            password: String::new(),
            bash_history: String::new(),
            metrics_history: DashMap::new(),
            last_update: None,
            services: DashMap::new(),
            hardware_info: serde_json::Value::Null,
            failed_logins: vec![],
        }
    }
}

// PerformanceOptimizer equivalent
#[derive(Debug)]
pub struct PerformanceOptimizer {
    pub query_cache: DashMap<String, serde_json::Value>,
    pub cache_ttl: DashMap<String, Instant>,
    pub max_connections: usize,
    pub cache_size: usize,
}

impl PerformanceOptimizer {
    pub fn new() -> Self {
        Self {
            query_cache: DashMap::new(),
            cache_ttl: DashMap::new(),
            max_connections: 50,
            cache_size: 1000,
        }
    }

    pub fn cache_query_result(&self, key: String, result: serde_json::Value, ttl: Duration) {
        if self.query_cache.len() >= self.cache_size {
            // Simple eviction: remove oldest
            if let Some(first_key) = self.query_cache.iter().next().map(|entry| entry.key().clone()) {
                self.query_cache.remove(&first_key);
                self.cache_ttl.remove(&first_key);
            }
        }
        self.query_cache.insert(key.clone(), result);
        self.cache_ttl.insert(key, Instant::now() + ttl);
    }

    pub fn get_cached_result(&self, key: &str) -> Option<serde_json::Value> {
        if let Some(ttl_entry) = self.cache_ttl.get(key) {
            if Instant::now() < *ttl_entry.value() {
                return self.query_cache.get(key).map(|v| v.clone());
            } else {
                self.query_cache.remove(key);
                self.cache_ttl.remove(key);
            }
        }
        None
    }
}

// AnomalyDetector with simple stats
#[derive(Debug)]
pub struct AnomalyDetector {
    pub baselines: DashMap<String, VecDeque<f64>>, // key: device_ip_metric -> history
    pub history_window: usize,
}

impl AnomalyDetector {
    pub fn new() -> Self {
        Self {
            baselines: DashMap::new(),
            history_window: 100,
        }
    }

    pub fn update_baseline(&self, device_ip: &str, metric: &str, value: f64) {
        let key = format!("{}_{}", device_ip, metric);
        let mut history = self.baselines.entry(key).or_insert_with(|| VecDeque::with_capacity(self.history_window));
        history.push_back(value);
        if history.len() > self.history_window {
            history.pop_front();
        }
    }

    pub fn detect_anomaly(&self, device_ip: &str, metric: &str, value: f64) -> bool {
        let key = format!("{}_{}", device_ip, metric);
        if let Some(history) = self.baselines.get(&key) {
            if history.len() < 10 {
                return false;
            }
            let data: Vec<f64> = history.iter().cloned().collect();
            let mean = data.iter().sum::<f64>() / data.len() as f64;
            let variance = data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / data.len() as f64;
            let std_dev = variance.sqrt();
            if std_dev > 0.0 {
                let z_score = ((value - mean) / std_dev).abs();
                return z_score > 2.0;
            }
        }
        false
    }
}

// Simplified AppHandle type
pub type AppHandle = Arc<Mutex<()>>;

// AlertManager
#[derive(Debug)]
pub struct AlertManager {
    pub thresholds: DashMap<String, AlertThreshold>,
    pub alerts: DashMap<String, Alert>,
    pub escalation_rules: Vec<String>, // Placeholder for rules
    pub anomaly_detector: Arc<AnomalyDetector>,
    pub app_handle: AppHandle, // Reference to app for GUI updates
}

impl AlertManager {
    pub fn new(app_handle: AppHandle) -> Self {
        let detector = Arc::new(AnomalyDetector::new());
        
        Self {
            thresholds: DashMap::new(),
            alerts: DashMap::new(),
            escalation_rules: vec![],
            anomaly_detector: detector,
            app_handle,
        }
    }

    pub fn set_threshold(&self, metric: &str, warning: f64, critical: f64, duration: u64) {
        self.thresholds.insert(metric.to_string(), AlertThreshold {
            metric: metric.to_string(),
            warning_level: warning,
            critical_level: critical,
            duration,
            enabled: true,
        });
    }

    pub fn check_metric(&self, device_ip: &str, metric: &str, value: f64) {
        if let Some(threshold) = self.thresholds.get(metric) {
            if !threshold.enabled {
                return;
            }
            self.anomaly_detector.update_baseline(device_ip, metric, value);
            
            // Check if there's an existing resolved alert
            let alert_key = format!("{}_{}", device_ip, metric);
            
            // Create a new alert if a condition is met
            if self.anomaly_detector.detect_anomaly(device_ip, metric, value) {
                // Fix: Use the consistent alert key to replace existing alerts
                self.create_alert(device_ip, metric, value, "anomaly".to_string(), 0.0, &alert_key);
            } else if value >= threshold.critical_level {
                // Fix: Use the consistent alert key to replace existing alerts
                self.create_alert(device_ip, metric, value, "critical".to_string(), threshold.critical_level, &alert_key);
            } else if value >= threshold.warning_level {
                // Fix: Use the consistent alert key to replace existing alerts
                self.create_alert(device_ip, metric, value, "warning".to_string(), threshold.warning_level, &alert_key);
            } else {
                // Resolve existing alert if the metric is back to normal
                if let Some(mut alert) = self.alerts.get_mut(&alert_key) {
                    alert.resolved = true;
                }
            }
        }
    }
    
    // Fix: Added a new `alert_key` parameter to create_alert
    pub fn create_alert(&self, device_ip: &str, metric: &str, value: f64, level: String, threshold: f64, alert_key: &str) {
        let alert = Alert {
            id: alert_key.to_string(), // Fix: Use the consistent key as the ID
            device_ip: device_ip.to_string(),
            metric: metric.to_string(),
            level,
            value,
            threshold,
            timestamp: Utc::now(),
            acknowledged: false,
            resolved: false,
        };
        self.alerts.insert(alert_key.to_string(), alert.clone());
        info!("Created alert: {} for device {}", alert_key, device_ip);
    }


    pub fn get_active_alerts(&self) -> Vec<Alert> {
        self.alerts
            .iter()
            .filter_map(|entry| {
                if !entry.resolved {
                    Some(entry.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

// DatabaseManager with proper connection handling
#[derive(Debug)]
pub struct DatabaseManager {
    pub connection: Arc<Mutex<Connection>>,
    pub db_path: String,
}

impl DatabaseManager {
    // Fix: Refactored `new` to prevent a deadlock
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute("PRAGMA journal_mode=WAL;", [])?;
        
        let mut db = Self {
            connection: Arc::new(Mutex::new(conn)),
            db_path: db_path.to_string(),
        };
        db.init_database()?;
        Ok(db)
    }

    fn init_database(&mut self) -> Result<()> {
        let conn = self.connection.lock().unwrap();
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS devices (
                mac TEXT PRIMARY KEY,
                ip TEXT,
                hostname TEXT,
                os_type TEXT,
                status TEXT,
                monitoring_enabled BOOLEAN,
                last_seen TIMESTAMP,
                hardware_info TEXT,
                services TEXT,
                username TEXT,
                password TEXT,
                bash_history TEXT
            )",
            [],
        )?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS metrics (
                id INTEGER PRIMARY KEY,
                device_mac TEXT,
                timestamp TIMESTAMP,
                cpu_percent REAL,
                memory_percent REAL,
                disk_percent REAL,
                network_bytes_sent INTEGER,
                network_bytes_recv INTEGER,
                load_avg_1 REAL,
                FOREIGN KEY (device_mac) REFERENCES devices(mac)
            )",
            [],
        )?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS alerts (
                id TEXT PRIMARY KEY,
                device_mac TEXT,
                metric TEXT,
                level TEXT,
                value REAL,
                threshold_value REAL,
                timestamp TIMESTAMP,
                acknowledged BOOLEAN DEFAULT FALSE,
                resolved BOOLEAN DEFAULT FALSE,
                FOREIGN KEY (device_mac) REFERENCES devices(mac)
            )",
            [],
        )?;
        
        // Indexes
        conn.execute("CREATE INDEX IF NOT EXISTS idx_metrics_device_time ON metrics(device_mac, timestamp)", [])?;
        Ok(())
    }

    pub fn save_device(&self, device: &NetworkDevice) -> Result<()> {
        let conn = self.connection.lock().unwrap();
        let hardware_json = serde_json::to_string(&device.hardware_info)?;
        let services_json = serde_json::to_string(&device.services.iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect::<HashMap<String, f64>>())?;
        
        conn.execute(
            "INSERT OR REPLACE INTO devices (mac, ip, hostname, os_type, status, monitoring_enabled, last_seen, hardware_info, services, username, password, bash_history) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                device.mac,
                device.ip,
                device.hostname,
                device.os_type,
                device.status,
                device.monitoring_enabled,
                device.last_update.map(|t| t.to_rfc3339()),
                hardware_json,
                services_json,
                device.username,
                device.password,
                device.bash_history
            ],
        )?;
        Ok(())
    }

    // Fix: `load_devices` now selects and deserializes all columns
    pub fn load_devices(&self) -> Result<DashMap<String, NetworkDevice>> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare("SELECT mac, ip, hostname, os_type, status, monitoring_enabled, username, password, bash_history, hardware_info, services, last_seen FROM devices")?;
        let devices = DashMap::new();
        let rows = stmt.query_map([], |row| {
            let hardware_json: String = row.get(9)?;
            let services_json: String = row.get(10)?;
            let last_seen_str: Option<String> = row.get(11)?;
            
            let hardware_info: serde_json::Value = serde_json::from_str(&hardware_json).unwrap_or(serde_json::Value::Null);
            let services_map: HashMap<String, f64> = serde_json::from_str(&services_json).unwrap_or_else(|_| HashMap::new());
            let services: DashMap<String, f64> = services_map.into_iter().collect();
            let last_update = last_seen_str.map(|s| {
                DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))
            }).flatten();
            
            Ok(NetworkDevice {
                mac: row.get(0)?,
                ip: row.get(1)?,
                hostname: row.get(2)?,
                os_type: row.get(3)?,
                status: row.get(4)?,
                monitoring_enabled: row.get(5)?,
                username: row.get(6)?,
                password: row.get(7)?,
                bash_history: row.get(8)?,
                hardware_info,
                services,
                last_update,
                ..Default::default()
            })
        })?;
        
        for device_result in rows {
            let device = device_result?;
            devices.insert(device.mac.clone(), device);
        }
        Ok(devices)
    }
}

// Simplified SSHManager
#[derive(Debug)]
pub struct SSHManager {
    pub optimizer: Arc<PerformanceOptimizer>,
    pub custom_scripts: DashMap<String, String>, // script_id -> script
}

impl SSHManager {
    pub fn new() -> Self {
        Self {
            optimizer: Arc::new(PerformanceOptimizer::new()),
            custom_scripts: DashMap::new(),
        }
    }

    pub async fn connect_to_device(&self, _device: &mut NetworkDevice) -> Result<bool> {
        // Simplified - mock connection
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(true)
    }

    pub async fn execute_command(&self, _device: &NetworkDevice, command: &str) -> Result<String> {
        // Mock implementation
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(format!("Mock output for: {}", command))
    }

    pub async fn get_enhanced_metrics(&self, _device: &NetworkDevice) -> Result<serde_json::Value> {
        // Mock metrics with realistic values
        let metrics = serde_json::json!({
            "cpu": 20.0 + rand::random::<f64>() * 60.0,
            "memory": 30.0 + rand::random::<f64>() * 50.0,
            "disk": 40.0 + rand::random::<f64>() * 40.0,
            "load_avg": [
                rand::random::<f64>() * 2.0,
                rand::random::<f64>() * 2.0,
                rand::random::<f64>() * 2.0
            ],
            "processes": 100 + (rand::random::<f64>() * 100.0) as i32,
            "top_services": {},
            "timestamp": Utc::now()
        });
        Ok(metrics)
    }
}

// NetworkScanner
#[derive(Debug)]
pub struct NetworkScanner {
    pub callback_tx: Option<mpsc::Sender<String>>, // For UI updates
}

impl NetworkScanner {
    pub fn new(tx: mpsc::Sender<String>) -> Self {
        Self {
            callback_tx: Some(tx)
        }
    }

    pub async fn scan_network(&self, _network_range: &str) -> Result<Vec<NetworkDevice>> {
        // Simplified mock scan
        let mut devices = vec![];
        for i in 1..=3 {
            let device = NetworkDevice {
                ip: format!("192.168.1.{}", i + 100),
                mac: format!("00:11:22:33:44:{:02x}", i),
                hostname: format!("device-{}", i),
                os_type: if i % 2 == 0 { "Ubuntu 22.04".to_string() } else { "Windows 11".to_string() },
                status: "Online".to_string(),
                monitoring_enabled: true,
                ..Default::default()
            };
            devices.push(device);
            
            if let Some(ref tx) = self.callback_tx {
                let _ = tx.try_send(format!("Found device: 192.168.1.{}", i + 100));
            }
            
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Ok(devices)
    }
}

// GUI with egui
#[derive(Debug)]
pub struct ModernGUI {
    pub current_theme: String, // "light" or "dark"
}

impl ModernGUI {
    pub fn new() -> Self {
        Self {
            current_theme: "light".to_string(),
        }
    }

    pub fn apply_theme(&mut self, ctx: &egui::Context, theme: &str) {
        self.current_theme = theme.to_string();
        let visuals = if theme == "dark" {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        ctx.set_visuals(visuals);
    }

    pub fn create_metric_card(&self, ui: &mut egui::Ui, title: &str, value: f64, unit: &str, color: egui::Color32) -> egui::Response {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(title).size(12.0).color(egui::Color32::GRAY));
                ui.label(egui::RichText::new(format!("{:.1}{}", value, unit)).size(20.0).color(color));
            });
        }).response
    }
}

// MonitoringThread
#[derive(Debug)]
pub struct MonitoringThread {
    pub running: Arc<AtomicBool>,
    pub interval: Duration,
}

impl MonitoringThread {
    pub fn new(interval_secs: u64) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            interval: Duration::from_secs(interval_secs),
        }
    }

    // Fix: The spawn logic is refactored to pass `Arc`s and handle `DashMap` mutability correctly
    pub fn start(&self, devices: Arc<DashMap<String, NetworkDevice>>, ssh_manager: Arc<SSHManager>, alert_manager: Arc<AlertManager>) {
        let running = self.running.clone();
        let interval = self.interval;
        
        let _handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            
            rt.block_on(async move {
                while running.load(Ordering::Relaxed) {
                    let device_keys: Vec<_> = devices.iter()
                        .filter(|d| d.monitoring_enabled)
                        .map(|d| d.key().clone())
                        .collect();
                    
                    for key in device_keys {
                        let devices_clone = devices.clone();
                        let ssh = ssh_manager.clone();
                        let alert = alert_manager.clone();
                        
                        tokio::spawn(async move {
                            // Fix: Use `get_mut` to get a mutable reference to the device in the DashMap
                            if let Some(mut device_ref) = devices_clone.get_mut(&key) {
                                let device = device_ref.value_mut();
                                
                                if ssh.connect_to_device(device).await.is_ok() {
                                    if let Ok(metrics) = ssh.get_enhanced_metrics(device).await {
                                        let cpu_value = metrics["cpu"].as_f64().unwrap_or(0.0);
                                        let memory_value = metrics["memory"].as_f64().unwrap_or(0.0);
                                        let disk_value = metrics["disk"].as_f64().unwrap_or(0.0);
                                        
                                        let timestamp = Utc::now();
                                        
                                        // Update metrics history directly
                                        for (metric_name, value) in [
                                            ("cpu", cpu_value),
                                            ("memory", memory_value),
                                            ("disk", disk_value)
                                        ] {
                                            let mut history = device.metrics_history
                                                .entry(metric_name.to_string())
                                                .or_insert_with(VecDeque::new);
                                            
                                            history.push_back((value, timestamp));
                                            if history.len() > 100 {
                                                history.pop_front();
                                            }
                                        }

                                        // Update the device's last_update timestamp
                                        device.last_update = Some(timestamp);
                                        
                                        // Check alerts
                                        alert.check_metric(&device.ip, "cpu", cpu_value);
                                        alert.check_metric(&device.ip, "memory", memory_value);
                                        alert.check_metric(&device.ip, "disk", disk_value);
                                    }
                                }
                            }
                        });
                    }
                    
                    tokio::time::sleep(interval).await;
                }
            });
        });
    }
}

// Update event types
#[derive(Debug, Clone)]
pub enum UpdateEvent {
    DeviceUpdate(NetworkDevice),
    AlertUpdate(Alert),
    Log(String),
}

// Main App struct for egui
#[derive(Debug)]
pub struct EnhancedPCManagementApp {
    pub devices: Arc<DashMap<String, NetworkDevice>>,
    pub db_manager: Arc<DatabaseManager>,
    pub ssh_manager: Arc<SSHManager>,
    pub scanner: Arc<NetworkScanner>,
    pub alert_manager: Arc<AlertManager>,
    pub monitoring_thread: MonitoringThread,
    pub modern_gui: ModernGUI,
    pub optimizer: Arc<PerformanceOptimizer>,
    // GUI state
    pub selected_device: Option<String>,
    pub search_term: String,
    pub network_range: String,
    pub alerts: Vec<Alert>,
    pub log_messages: VecDeque<String>,
    // Plot state
    pub cpu_data: Vec<(f64, f64)>, // time, value
    // Channels for UI updates
    pub log_tx: mpsc::Sender<String>,
    pub log_rx: mpsc::Receiver<String>,
    // Fix: `is_scanning` is now an AtomicBool for thread-safe access
    pub is_scanning: Arc<AtomicBool>,
}

impl EnhancedPCManagementApp {
    pub fn new() -> Result<Self> {
        let (log_tx, log_rx) = mpsc::channel(100);
        
        let devices = Arc::new(DashMap::new());
        let ssh = Arc::new(SSHManager::new());
        let app_handle = Arc::new(Mutex::new(()));
        let alert_mgr = Arc::new(AlertManager::new(app_handle));
        let scanner = Arc::new(NetworkScanner::new(log_tx.clone()));
        let monitoring = MonitoringThread::new(5);
        let gui = ModernGUI::new();
        let optimizer = Arc::new(PerformanceOptimizer::new());
        let is_scanning = Arc::new(AtomicBool::new(false));

        // Load devices from DB
        let db = Arc::new(DatabaseManager::new("network_monitor.db")?);
        if let Ok(loaded_devices) = db.load_devices() {
            for entry in loaded_devices {
                devices.insert(entry.0, entry.1);
            }
        }

        // Set up default alert thresholds
        alert_mgr.set_threshold("cpu", 80.0, 95.0, 300);
        alert_mgr.set_threshold("memory", 85.0, 95.0, 300);
        alert_mgr.set_threshold("disk", 90.0, 98.0, 600);

        Ok(Self {
            devices,
            db_manager: db,
            ssh_manager: ssh,
            scanner,
            alert_manager: alert_mgr,
            monitoring_thread: monitoring,
            modern_gui: gui,
            optimizer,
            selected_device: None,
            search_term: String::new(),
            network_range: "192.168.1.0/24".to_string(),
            alerts: vec![],
            log_messages: VecDeque::new(),
            log_tx,
            log_rx,
            cpu_data: vec![],
            is_scanning,
        })
    }

    pub fn start_monitoring(&self) {
        self.monitoring_thread.running.store(true, Ordering::Relaxed);
        self.monitoring_thread.start(
            self.devices.clone(),
            self.ssh_manager.clone(),
            self.alert_manager.clone(),
        );
    }

    // Fix: Refactored to use the AtomicBool for a thread-safe scanning flag
    pub fn start_network_scan(&self) {
        if self.is_scanning.load(Ordering::Relaxed) {
            return;
        }
        
        self.is_scanning.store(true, Ordering::Relaxed);
        
        let scanner = self.scanner.clone();
        let devices = self.devices.clone();
        let db = self.db_manager.clone();
        let range = self.network_range.clone();
        let log_tx = self.log_tx.clone();
        let is_scanning_flag = self.is_scanning.clone();
        
        let rt = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            rt.block_on(async {
                let _ = log_tx.send("Starting network scan...".to_string()).await;
                
                match scanner.scan_network(&range).await {
                    Ok(found_devices) => {
                        let _ = log_tx.send(format!("Found {} devices", found_devices.len())).await;
                        
                        for device in found_devices {
                            devices.insert(device.mac.clone(), device.clone());
                            
                            // Save to database
                            if let Err(e) = db.save_device(&device) {
                                error!("Failed to save device {}: {}", device.ip, e);
                            }
                        }
                        
                        let _ = log_tx.send("Network scan completed".to_string()).await;
                    }
                    Err(e) => {
                        let _ = log_tx.send(format!("Network scan failed: {}", e)).await;
                        error!("Network scan error: {}", e);
                    }
                }
                
                is_scanning_flag.store(false, Ordering::Relaxed);
            });
        });
    }

    pub fn refresh_alerts(&mut self) {
        self.alerts = self.alert_manager.get_active_alerts();
    }

    pub fn process_log_messages(&mut self) {
        while let Ok(msg) = self.log_rx.try_recv() {
            self.log_messages.push_back(msg);
            if self.log_messages.len() > 100 {
                self.log_messages.pop_front();
            }
        }
    }
}

impl eframe::App for EnhancedPCManagementApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process incoming log messages
        self.process_log_messages();

        // Refresh alerts periodically
        self.refresh_alerts();

        // Theme toggle
        if ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.ctrl) {
            let new_theme = if self.modern_gui.current_theme == "light" {
                "dark".to_string()
            } else {
                "light".to_string()
            };
            self.modern_gui.apply_theme(ctx, &new_theme);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Toolbar
            ui.horizontal(|ui| {
                if ui.button("🔍 Quick Scan").clicked() {
                    self.start_network_scan();
                }
                
                ui.label("Search:");
                ui.text_edit_singleline(&mut self.search_term);
                
                ui.label("Network Range:");
                ui.text_edit_singleline(&mut self.network_range);
                
                ui.separator();
                ui.label(format!("Theme: {}", self.modern_gui.current_theme));
                ui.label("Press Ctrl+T to toggle theme");
                
                // Display scanning status
                ui.label(if self.is_scanning.load(Ordering::Relaxed) {
                    "Status: Scanning..."
                } else {
                    "Status: Ready"
                });
            });

            ui.separator();

            // Main content area
            ui.horizontal(|ui| {
                // Left panel: Device list
                ui.vertical(|ui| {
                    ui.heading("Devices");
                    ui.label(format!("Total: {}", self.devices.len()));
                    ui.separator();
                    
                    egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                        let devices_clone = self.devices.clone(); // Clone Arc for iterating
                        let filtered_devices: Vec<_> = devices_clone.iter()
                            .filter(|entry| {
                                let device_label = format!("{} - {} ({})", entry.ip, entry.hostname, entry.os_type);
                                self.search_term.is_empty() || device_label.to_lowercase().contains(&self.search_term.to_lowercase())
                            })
                            .collect();

                        for entry in filtered_devices {
                            let device = entry.value();
                            let device_label = format!("{} - {} ({})", device.ip, device.hostname, device.os_type);
                            
                            let is_selected = self.selected_device.as_ref().map_or(false, |mac| mac == entry.key());
                            
                            if ui.selectable_label(is_selected, device_label).clicked() {
                                self.selected_device = Some(entry.key().clone());
                            }
                        }
                    });
                });

                ui.separator();

                // Right panel: Device details and monitoring
                ui.vertical(|ui| {
                    if let Some(selected_key) = &self.selected_device {
                        if let Some(device_ref) = self.devices.get(selected_key) {
                            let device = device_ref.value();
                            
                            ui.heading(&format!("Device: {}", device.hostname));
                            
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(&format!("IP: {}", device.ip));
                                    ui.label(&format!("MAC: {}", device.mac));
                                    ui.label(&format!("OS: {}", device.os_type));
                                });
                                
                                ui.vertical(|ui| {
                                    ui.label(&format!("Status: {}", device.status));
                                    ui.label(&format!("Monitoring: {}",
                                        if device.monitoring_enabled { "Enabled" } else { "Disabled" }));
                                });
                            });
                            
                            ui.separator();
                            
                            // Real-time metrics from device history
                            ui.heading("Live Metrics");
                            
                            ui.horizontal(|ui| {
                                // Get latest metrics from device history
                                let cpu_latest = device.metrics_history.get("cpu")
                                    .and_then(|history| history.back().map(|(value, _)| *value))
                                    .unwrap_or(0.0);
                                    
                                let memory_latest = device.metrics_history.get("memory")
                                    .and_then(|history| history.back().map(|(value, _)| *value))
                                    .unwrap_or(0.0);
                                    
                                let disk_latest = device.metrics_history.get("disk")
                                    .and_then(|history| history.back().map(|(value, _)| *value))
                                    .unwrap_or(0.0);
                                    
                                self.modern_gui.create_metric_card(
                                    ui,
                                    "CPU Usage",
                                    cpu_latest,
                                    "%",
                                    if cpu_latest > 95.0 { egui::Color32::RED }
                                    else if cpu_latest > 80.0 { egui::Color32::YELLOW }
                                    else { egui::Color32::GREEN }
                                );
                                
                                ui.separator();
                                
                                self.modern_gui.create_metric_card(
                                    ui,
                                    "Memory",
                                    memory_latest,
                                    "%",
                                    if memory_latest > 95.0 { egui::Color32::RED }
                                    else if memory_latest > 85.0 { egui::Color32::YELLOW }
                                    else { egui::Color32::GREEN }
                                );
                                
                                ui.separator();
                                
                                self.modern_gui.create_metric_card(
                                    ui,
                                    "Disk",
                                    disk_latest,
                                    "%",
                                    if disk_latest > 98.0 { egui::Color32::RED }
                                    else if disk_latest > 90.0 { egui::Color32::YELLOW }
                                    else { egui::Color32::GREEN }
                                );
                            });
                            
                            ui.separator();
                            
                            // Plot CPU history
                            if let Some(cpu_history) = device.metrics_history.get("cpu") {
                                if !cpu_history.is_empty() {
                                    ui.heading("CPU History");
                                    
                                    use egui_plot::*;
                                    let plot_points: PlotPoints = cpu_history.iter()
                                        .enumerate()
                                        .map(|(i, (value, _timestamp))| [i as f64, *value])
                                        .collect();
                                    
                                    Plot::new("cpu_history")
                                        .view_aspect(2.0)
                                        .height(200.0)
                                        .show(ui, |plot_ui| {
                                            plot_ui.line(Line::new(plot_points).color(egui::Color32::BLUE));
                                        });
                                }
                            }
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(50.0);
                            ui.heading("No Device Selected");
                            ui.label("Select a device from the list to view details and metrics");
                        });
                    }
                });
            });

            ui.separator();

            // Bottom panel: Logs and alerts
            ui.horizontal(|ui| {
                // Logs panel
                ui.vertical(|ui| {
                    ui.heading("System Logs");
                    ui.separator();
                    
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for msg in &self.log_messages {
                                ui.label(msg);
                            }
                            
                            if self.log_messages.is_empty() {
                                ui.label("No log messages");
                            }
                        });
                });

                ui.separator();

                // Alerts panel
                ui.vertical(|ui| {
                    ui.heading("Active Alerts");
                    ui.label(format!("Count: {}", self.alerts.len()));
                    ui.separator();
                    
                    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                        if self.alerts.is_empty() {
                            ui.label("No active alerts");
                        } else {
                            for alert in &self.alerts {
                                let color = match alert.level.as_str() {
                                    "critical" => egui::Color32::RED,
                                    "warning" => egui::Color32::YELLOW,
                                    "anomaly" => egui::Color32::BLUE,
                                    _ => egui::Color32::WHITE,
                                };
                                
                                ui.horizontal(|ui| {
                                    ui.colored_label(color, &alert.level.to_uppercase());
                                    ui.label(format!("{}: {} = {:.1}",
                                        alert.device_ip, alert.metric, alert.value));
                                });
                            }
                        }
                    });
                });
            });
        });

        // Request repaint for live updates
        ctx.request_repaint_after(Duration::from_secs(2));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Enhanced Network PC Management Application");

    // Set up egui options
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Enhanced Network PC Management v2.0"),
        ..Default::default()
    };

    // Create and start the application
    let result = eframe::run_native(
        "Enhanced Network PC Management v2.0",
        options,
        Box::new(|_cc| {
            match EnhancedPCManagementApp::new() {
                Ok(mut app) => {
                    // Add some sample devices for demonstration
                    let sample_devices = vec![
                        NetworkDevice {
                            ip: "192.168.1.100".to_string(),
                            mac: "00:11:22:33:44:55".to_string(),
                            hostname: "workstation-01".to_string(),
                            os_type: "Ubuntu 22.04".to_string(),
                            status: "Online".to_string(),
                            monitoring_enabled: true,
                            username: "admin".to_string(),
                            ..Default::default()
                        },
                        NetworkDevice {
                            ip: "192.168.1.101".to_string(),
                            mac: "00:11:22:33:44:56".to_string(),
                            hostname: "server-01".to_string(),
                            os_type: "Windows Server 2022".to_string(),
                            status: "Online".to_string(),
                            monitoring_enabled: true,
                            username: "administrator".to_string(),
                            ..Default::default()
                        },
                        NetworkDevice {
                            ip: "192.168.1.102".to_string(),
                            mac: "00:11:22:33:44:57".to_string(),
                            hostname: "laptop-01".to_string(),
                            os_type: "macOS Sonoma".to_string(),
                            status: "Online".to_string(),
                            monitoring_enabled: false,
                            username: "user".to_string(),
                            ..Default::default()
                        },
                    ];
                    
                    for mut device in sample_devices {
                        // Add some sample metrics history
                        let mut cpu_history = VecDeque::new();
                        let mut memory_history = VecDeque::new();
                        let mut disk_history = VecDeque::new();
                        
                        let now = Utc::now();
                        for i in 0..20 {
                            let timestamp = now - chrono::Duration::minutes(i);
                            cpu_history.push_front((
                                30.0 + 25.0 * ((i as f64) * 0.3).sin() + rand::random::<f64>() * 10.0,
                                timestamp
                            ));
                            memory_history.push_front((
                                45.0 + 20.0 * ((i as f64) * 0.2).cos() + rand::random::<f64>() * 8.0,
                                timestamp
                            ));
                            disk_history.push_front((
                                60.0 + 15.0 * ((i as f64) * 0.1).sin() + rand::random::<f64>() * 5.0,
                                timestamp
                            ));
                        }
                        
                        device.metrics_history.insert("cpu".to_string(), cpu_history);
                        device.metrics_history.insert("memory".to_string(), memory_history);
                        device.metrics_history.insert("disk".to_string(), disk_history);
                        device.last_update = Some(now); // Set last_update for sample devices
                        
                        app.devices.insert(device.mac.clone(), device.clone());
                        
                        // Save sample device to database
                        if let Err(e) = app.db_manager.save_device(&device) {
                            error!("Failed to save sample device: {}", e);
                        }
                    }
                    
                    // Start monitoring
                    app.start_monitoring();
                    
                    info!("Application initialized successfully with {} devices", app.devices.len());
                    Box::new(app)
                }
                Err(e) => {
                    error!("Failed to initialize application: {}", e);
                    panic!("Application initialization failed: {}", e);
                }
            }
        }),
    );

    match result {
        Ok(_) => {
            info!("Application closed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Application error: {}", e);
            Ok(()) // Don't propagate eframe errors as they're not compatible with anyhow
        }
    }
}
