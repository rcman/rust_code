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
r2d2 = "0.8"
r2d2_sqlite = "0.22"
*/

use anyhow::{Result, Context, anyhow};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use eframe::egui;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn, Level};

// Enhanced configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub monitoring_interval: u64,
    pub max_history_size: usize,
    pub cache_ttl_seconds: u64,
    pub max_concurrent_monitors: usize,
    pub database_path: String,
    pub alert_thresholds: HashMap<String, (f64, f64)>, // metric -> (warning, critical)
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut thresholds = HashMap::new();
        thresholds.insert("cpu".to_string(), (80.0, 95.0));
        thresholds.insert("memory".to_string(), (85.0, 95.0));
        thresholds.insert("disk".to_string(), (90.0, 98.0));
        
        Self {
            monitoring_interval: 5,
            max_history_size: 100,
            cache_ttl_seconds: 300,
            max_concurrent_monitors: 10,
            database_path: "network_monitor.db".to_string(),
            alert_thresholds: thresholds,
        }
    }
}

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
    pub message: String,
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
    pub password: String, // TODO: Encrypt this
    pub bash_history: String,
    pub metrics_history: Arc<RwLock<HashMap<String, VecDeque<(f64, DateTime<Utc>)>>>>,
    pub last_update: Option<DateTime<Utc>>,
    pub services: Arc<DashMap<String, f64>>,
    pub hardware_info: serde_json::Value,
    pub failed_logins: Vec<DateTime<Utc>>,
    pub connection_errors: u32,
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
            metrics_history: Arc::new(RwLock::new(HashMap::new())),
            last_update: None,
            services: Arc::new(DashMap::new()),
            hardware_info: serde_json::Value::Null,
            failed_logins: vec![],
            connection_errors: 0,
        }
    }
}

// Enhanced PerformanceOptimizer
#[derive(Debug)]
pub struct PerformanceOptimizer {
    pub query_cache: Arc<DashMap<String, (serde_json::Value, Instant)>>,
    pub max_connections: usize,
    pub cache_size: usize,
    pub cache_ttl: Duration,
}

impl PerformanceOptimizer {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            query_cache: Arc::new(DashMap::new()),
            max_connections: 50,
            cache_size: 1000,
            cache_ttl: Duration::from_secs(config.cache_ttl_seconds),
        }
    }

    pub fn cache_query_result(&self, key: String, result: serde_json::Value) {
        // Evict expired entries
        self.cleanup_expired_cache();
        
        if self.query_cache.len() >= self.cache_size {
            // Remove oldest entry
            if let Some(entry) = self.query_cache.iter().next() {
                let key_to_remove = entry.key().clone();
                self.query_cache.remove(&key_to_remove);
            }
        }
        
        self.query_cache.insert(key, (result, Instant::now()));
    }

    pub fn get_cached_result(&self, key: &str) -> Option<serde_json::Value> {
        if let Some(entry) = self.query_cache.get(key) {
            let (value, timestamp) = entry.value();
            if timestamp.elapsed() < self.cache_ttl {
                return Some(value.clone());
            } else {
                drop(entry);
                self.query_cache.remove(key);
            }
        }
        None
    }

    fn cleanup_expired_cache(&self) {
        let expired_keys: Vec<_> = self.query_cache
            .iter()
            .filter_map(|entry| {
                if entry.value().1.elapsed() > self.cache_ttl {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();

        for key in expired_keys {
            self.query_cache.remove(&key);
        }
    }
}

// Enhanced AnomalyDetector with statistical methods
#[derive(Debug)]
pub struct AnomalyDetector {
    pub baselines: Arc<DashMap<String, VecDeque<f64>>>,
    pub history_window: usize,
    pub z_score_threshold: f64,
}

impl AnomalyDetector {
    pub fn new(history_window: usize) -> Self {
        Self {
            baselines: Arc::new(DashMap::new()),
            history_window,
            z_score_threshold: 2.5, // More conservative threshold
        }
    }

    pub fn update_baseline(&self, device_ip: &str, metric: &str, value: f64) {
        let key = format!("{}_{}", device_ip, metric);
        let mut history = self.baselines
            .entry(key)
            .or_insert_with(|| VecDeque::with_capacity(self.history_window));
        
        history.push_back(value);
        if history.len() > self.history_window {
            history.pop_front();
        }
    }

    pub fn detect_anomaly(&self, device_ip: &str, metric: &str, value: f64) -> (bool, f64) {
        let key = format!("{}_{}", device_ip, metric);
        if let Some(history) = self.baselines.get(&key) {
            if history.len() < 20 { // Need more data for reliable detection
                return (false, 0.0);
            }
            
            let data: Vec<f64> = history.iter().cloned().collect();
            let mean = data.iter().sum::<f64>() / data.len() as f64;
            let variance = data.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / data.len() as f64;
            let std_dev = variance.sqrt();
            
            if std_dev > 0.001 { // Avoid division by zero
                let z_score = ((value - mean) / std_dev).abs();
                return (z_score > self.z_score_threshold, z_score);
            }
        }
        (false, 0.0)
    }

    pub fn get_baseline_stats(&self, device_ip: &str, metric: &str) -> Option<(f64, f64, f64)> {
        let key = format!("{}_{}", device_ip, metric);
        if let Some(history) = self.baselines.get(&key) {
            if history.len() < 10 {
                return None;
            }
            
            let data: Vec<f64> = history.iter().cloned().collect();
            let mean = data.iter().sum::<f64>() / data.len() as f64;
            let variance = data.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / data.len() as f64;
            let std_dev = variance.sqrt();
            
            return Some((mean, std_dev, variance));
        }
        None
    }
}

// Enhanced AlertManager with better deduplication
#[derive(Debug)]
pub struct AlertManager {
    pub thresholds: Arc<DashMap<String, AlertThreshold>>,
    pub alerts: Arc<DashMap<String, Alert>>,
    pub escalation_rules: Vec<String>,
    pub anomaly_detector: Arc<AnomalyDetector>,
    pub alert_history: Arc<RwLock<VecDeque<Alert>>>,
    pub config: AppConfig,
}

impl AlertManager {
    pub fn new(config: AppConfig) -> Self {
        let detector = Arc::new(AnomalyDetector::new(config.max_history_size));
        
        let mut manager = Self {
            thresholds: Arc::new(DashMap::new()),
            alerts: Arc::new(DashMap::new()),
            escalation_rules: vec![],
            anomaly_detector: detector,
            alert_history: Arc::new(RwLock::new(VecDeque::new())),
            config,
        };
        
        // Initialize thresholds from config
        for (metric, (warning, critical)) in &manager.config.alert_thresholds {
            manager.set_threshold(metric, *warning, *critical, 300);
        }
        
        manager
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

    pub async fn check_metric(&self, device_ip: &str, metric: &str, value: f64) -> Result<()> {
        if let Some(threshold) = self.thresholds.get(metric) {
            if !threshold.enabled {
                return Ok(());
            }
            
            self.anomaly_detector.update_baseline(device_ip, metric, value);
            let alert_key = format!("{}_{}", device_ip, metric);
            
            // Determine alert level
            let (is_anomaly, z_score) = self.anomaly_detector.detect_anomaly(device_ip, metric, value);
            
            let new_level_info = if is_anomaly {
                Some(("anomaly", 0.0, format!("Anomalous {} value detected (z-score: {:.2})", metric, z_score)))
            } else if value >= threshold.critical_level {
                Some(("critical", threshold.critical_level, format!("{} usage critically high: {:.1}%", metric, value)))
            } else if value >= threshold.warning_level {
                Some(("warning", threshold.warning_level, format!("{} usage high: {:.1}%", metric, value)))
            } else {
                None
            };
            
            // Handle alert state changes
            match (self.alerts.get(&alert_key), new_level_info) {
                (Some(mut alert), Some((level, threshold_val, message))) => {
                    // Update existing alert if level changed or was resolved
                    if alert.level != level || alert.resolved {
                        alert.level = level.to_string();
                        alert.value = value;
                        alert.threshold = threshold_val;
                        alert.timestamp = Utc::now();
                        alert.message = message;
                        alert.resolved = false;
                        
                        info!("Updated alert: {} for device {}", alert_key, device_ip);
                    }
                }
                (Some(mut alert), None) => {
                    // Resolve existing alert
                    if !alert.resolved {
                        alert.resolved = true;
                        alert.timestamp = Utc::now();
                        alert.message = format!("{} returned to normal levels", metric);
                        
                        // Archive resolved alert
                        self.archive_alert(alert.clone()).await?;
                        info!("Resolved alert: {} for device {}", alert_key, device_ip);
                    }
                }
                (None, Some((level, threshold_val, message))) => {
                    // Create new alert
                    self.create_alert(device_ip, metric, value, level.to_string(), threshold_val, &alert_key, message).await?;
                }
                (None, None) => {
                    // No action needed
                }
            }
        }
        Ok(())
    }

    async fn create_alert(&self, device_ip: &str, metric: &str, value: f64, level: String, threshold: f64, alert_key: &str, message: String) -> Result<()> {
        let alert = Alert {
            id: alert_key.to_string(),
            device_ip: device_ip.to_string(),
            metric: metric.to_string(),
            level: level.clone(),
            value,
            threshold,
            timestamp: Utc::now(),
            acknowledged: false,
            resolved: false,
            message,
        };
        
        self.alerts.insert(alert_key.to_string(), alert.clone());
        info!("Created {} alert: {} for device {}", level, alert_key, device_ip);
        
        Ok(())
    }

    async fn archive_alert(&self, alert: Alert) -> Result<()> {
        let mut history = self.alert_history.write().await;
        history.push_back(alert);
        
        // Keep only last 1000 alerts in memory
        if history.len() > 1000 {
            history.pop_front();
        }
        
        Ok(())
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

    pub fn acknowledge_alert(&self, alert_id: &str) -> Result<()> {
        if let Some(mut alert) = self.alerts.get_mut(alert_id) {
            alert.acknowledged = true;
            info!("Acknowledged alert: {}", alert_id);
            Ok(())
        } else {
            Err(anyhow!("Alert not found: {}", alert_id))
        }
    }
}

// Enhanced DatabaseManager with connection pooling
#[derive(Debug)]
pub struct DatabaseManager {
    pub pool: Arc<Mutex<Vec<Connection>>>,
    pub db_path: String,
    pub max_connections: usize,
}

impl DatabaseManager {
    pub fn new(db_path: &str, max_connections: usize) -> Result<Self> {
        let mut connections = Vec::with_capacity(max_connections);
        
        // Create connection pool
        for _ in 0..max_connections {
            let conn = Connection::open(db_path)
                .with_context(|| format!("Failed to open database: {}", db_path))?;
            conn.execute("PRAGMA journal_mode=WAL;", [])
                .context("Failed to set WAL mode")?;
            conn.execute("PRAGMA synchronous=NORMAL;", [])
                .context("Failed to set synchronous mode")?;
            conn.execute("PRAGMA cache_size=10000;", [])
                .context("Failed to set cache size")?;
            connections.push(conn);
        }
        
        let mut db = Self {
            pool: Arc::new(Mutex::new(connections)),
            db_path: db_path.to_string(),
            max_connections,
        };
        
        db.init_database()?;
        Ok(db)
    }

    pub fn with_connection<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R>,
    {
        let mut pool = self.pool.lock()
            .map_err(|_| anyhow!("Database pool lock poisoned"))?;
        
        if let Some(conn) = pool.pop() {
            let result = f(&conn);
            pool.push(conn);
            result
        } else {
            // All connections busy, create temporary one
            warn!("All database connections busy, creating temporary connection");
            let conn = Connection::open(&self.db_path)
                .with_context(|| format!("Failed to create temporary connection to: {}", self.db_path))?;
            f(&conn)
        }
    }

    fn init_database(&mut self) -> Result<()> {
        self.with_connection(|conn| {
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
                    bash_history TEXT,
                    connection_errors INTEGER DEFAULT 0
                )",
                [],
            ).context("Failed to create devices table")?;
            
            conn.execute(
                "CREATE TABLE IF NOT EXISTS metrics (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
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
            ).context("Failed to create metrics table")?;
            
            conn.execute(
                "CREATE TABLE IF NOT EXISTS alerts (
                    id TEXT PRIMARY KEY,
                    device_ip TEXT,
                    metric TEXT,
                    level TEXT,
                    value REAL,
                    threshold_value REAL,
                    timestamp TIMESTAMP,
                    acknowledged BOOLEAN DEFAULT FALSE,
                    resolved BOOLEAN DEFAULT FALSE,
                    message TEXT,
                    INDEX(device_ip, timestamp),
                    INDEX(resolved, timestamp)
                )",
                [],
            ).context("Failed to create alerts table")?;
            
            // Create indexes
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_metrics_device_time ON metrics(device_mac, timestamp)",
                []
            ).context("Failed to create metrics index")?;
            
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_alerts_device_resolved ON alerts(device_ip, resolved)",
                []
            ).context("Failed to create alerts index")?;
            
            Ok(())
        })
    }

    pub fn save_device(&self, device: &NetworkDevice) -> Result<()> {
        self.with_connection(|conn| {
            let hardware_json = serde_json::to_string(&device.hardware_info)
                .context("Failed to serialize hardware info")?;
            
            // Convert services to serializable format
            let services: HashMap<String, f64> = device.services.iter()
                .map(|entry| (entry.key().clone(), *entry.value()))
                .collect();
            let services_json = serde_json::to_string(&services)
                .context("Failed to serialize services")?;
            
            conn.execute(
                "INSERT OR REPLACE INTO devices 
                (mac, ip, hostname, os_type, status, monitoring_enabled, last_seen, 
                 hardware_info, services, username, password, bash_history, connection_errors) 
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                    device.password, // TODO: Encrypt
                    device.bash_history,
                    device.connection_errors
                ],
            ).context("Failed to save device to database")?;
            
            Ok(())
        })
    }

    pub fn load_devices(&self) -> Result<HashMap<String, NetworkDevice>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT mac, ip, hostname, os_type, status, monitoring_enabled, 
                 username, password, bash_history, hardware_info, services, 
                 last_seen, connection_errors FROM devices"
            ).context("Failed to prepare device query")?;
            
            let mut devices = HashMap::new();
            let rows = stmt.query_map([], |row| {
                let hardware_json: String = row.get(9)?;
                let services_json: String = row.get(10)?;
                let last_seen_str: Option<String> = row.get(11)?;
                let connection_errors: u32 = row.get(12)?;
                
                let hardware_info: serde_json::Value = serde_json::from_str(&hardware_json)
                    .unwrap_or(serde_json::Value::Null);
                let services_map: HashMap<String, f64> = serde_json::from_str(&services_json)
                    .unwrap_or_else(|_| HashMap::new());
                let services: Arc<DashMap<String, f64>> = Arc::new(services_map.into_iter().collect());
                
                let last_update = last_seen_str
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc));
                
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
                    connection_errors,
                    ..Default::default()
                })
            }).context("Failed to query devices")?;
            
            for device_result in rows {
                let device = device_result.context("Failed to parse device row")?;
                devices.insert(device.mac.clone(), device);
            }
            
            Ok(devices)
        })
    }

    pub fn save_alert(&self, alert: &Alert) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO alerts 
                (id, device_ip, metric, level, value, threshold_value, timestamp, 
                 acknowledged, resolved, message) 
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    alert.id,
                    alert.device_ip,
                    alert.metric,
                    alert.level,
                    alert.value,
                    alert.threshold,
                    alert.timestamp.to_rfc3339(),
                    alert.acknowledged,
                    alert.resolved,
                    alert.message
                ],
            ).context("Failed to save alert to database")?;
            
            Ok(())
        })
    }
}

// Enhanced SSHManager with better error handling
#[derive(Debug)]
pub struct SSHManager {
    pub optimizer: Arc<PerformanceOptimizer>,
    pub custom_scripts: Arc<DashMap<String, String>>,
    pub connection_timeout: Duration,
    pub command_timeout: Duration,
}

impl SSHManager {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            optimizer: Arc::new(PerformanceOptimizer::new(config)),
            custom_scripts: Arc::new(DashMap::new()),
            connection_timeout: Duration::from_secs(10),
            command_timeout: Duration::from_secs(30),
        }
    }

    pub async fn connect_to_device(&self, device: &mut NetworkDevice) -> Result<bool> {
        // Check cache first
        let cache_key = format!("connection_{}", device.ip);
        if let Some(cached) = self.optimizer.get_cached_result(&cache_key) {
            if let Some(success) = cached.as_bool() {
                return Ok(success);
            }
        }
        
        // Simulate connection with realistic delay and error handling
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Simulate occasional connection failures
        let success = if device.connection_errors > 3 {
            rand::random::<f64>() > 0.7 // Higher failure rate for problematic devices
        } else {
            rand::random::<f64>() > 0.1 // 10% failure rate normally
        };
        
        if success {
            device.connection_errors = 0;
            device.status = "Online".to_string();
        } else {
            device.connection_errors += 1;
            device.status = "Connection Failed".to_string();
            warn!("Failed to connect to device: {} (errors: {})", device.ip, device.connection_errors);
        }
        
        // Cache result
        self.optimizer.cache_query_result(cache_key, serde_json::json!(success));
        
        Ok(success)
    }

    pub async fn execute_command(&self, device: &NetworkDevice, command: &str) -> Result<String> {
        let cache_key = format!("cmd_{}_{}", device.ip, command);
        if let Some(cached) = self.optimizer.get_cached_result(&cache_key) {
            if let Some(output) = cached.as_str() {
                return Ok(output.to_string());
            }
        }
        
        // Simulate command execution
        let timeout = tokio::time::timeout(self.command_timeout, async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(format!("Mock output for '{}' on {}", command, device.ip))
        }).await;
        
        match timeout {
            Ok(result) => {
                let output = result?;
                self.optimizer.cache_query_result(cache_key, serde_json::json!(output));
                Ok(output)
            }
            Err(_) => {
                Err(anyhow!("Command timeout: {} on {}", command, device.ip))
            }
        }
    }

    pub async fn get_enhanced_metrics(&self, device: &NetworkDevice) -> Result<serde_json::Value> {
        let cache_key = format!("metrics_{}", device.ip);
        if let Some(cached) = self.optimizer.get_cached_result(&cache_key) {
            return Ok(cached);
        }
        
        // Simulate getting metrics with realistic values and variations
        let base_time = Utc::now().timestamp() as f64;
        let cpu_base = 20.0 + 30.0 * ((base_time * 0.001).sin());
        let memory_base = 40.0 + 25.0 * ((base_time * 0.0008).cos());
        let disk_base = 60.0 + 15.0 * ((base_time * 0.0005).sin());
        
        let metrics = serde_json::json!({
            "cpu": (cpu_base + rand::random::<f64>() * 15.0).max(0.0).min(100.0),
            "memory": (memory_base + rand::random::<f64>() * 10.0).max(0.0).min(100.0),
            "disk": (disk_base + rand::random::<f64>() * 8.0).max(0.0).min(100.0),
            "load_avg": [
                rand::random::<f64>() * 3.0,
                rand::random::<f64>() * 2.5,
                rand::random::<f64>() * 2.0
            ],
            "processes": 80 + (rand::random::<f64>() * 120.0) as i32,
            "network_bytes_sent": (rand::random::<f64>() * 1000000.0) as u64,
            "network_bytes_recv": (rand::random::<f64>() * 1000000.0) as u64,
            "top_services": {
                "systemd": rand::random::<f64>() * 5.0,
                "chrome": rand::random::<f64>() * 20.0,
                "mysql": rand::random::<f64>() * 15.0
            },
            "timestamp": Utc::now()
        });
        
        self.optimizer.cache_query_result(cache_key, metrics.clone());
        Ok(metrics)
    }

    pub fn add_custom_script(&self, name: String, script: String) {
        self.custom_scripts.insert(name, script);
    }

    pub async fn execute_custom_script(&self, device: &NetworkDevice, script_name: &str) -> Result<String> {
        if let Some(script) = self.custom_scripts.get(script_name) {
            self.execute_command(device, &script).await
        } else {
            Err(anyhow!("Script not found: {}", script_name))
        }
    }
}

// Enhanced NetworkScanner with better error handling
#[derive(Debug)]
pub struct NetworkScanner {
    pub callback_tx: Option<mpsc::Sender<String>>,
    pub scan_timeout: Duration,
    pub max_concurrent_scans: usize,
}

impl NetworkScanner {
    pub fn new(tx: mpsc::Sender<String>) -> Self {
        Self {
            callback_tx: Some(tx),
            scan_timeout: Duration::from_secs(30),
            max_concurrent_scans: 50,
        }
    }

    pub async fn scan_network(&self, network_range: &str) -> Result<Vec<NetworkDevice>> {
        info!("Starting network scan of range: {}", network_range);
        
        let mut devices = vec![];
        let scan_tasks = Vec::new();
        
        // For demonstration, we'll create mock devices
        // In a real implementation, you would parse the network range and scan IP addresses
        for i in 1..=5 {
            let ip = format!("192.168.1.{}", i + 100);
            
            if let Some(ref tx) = self.callback_tx {
                let _ = tx.try_send(format!("Scanning {}", ip));
            }
            
            // Simulate network discovery delay
            tokio::time::sleep(Duration::from_millis(200)).await;
            
            // Simulate some devices being found
            if rand::random::<f64>() > 0.3 {
                let device = NetworkDevice {
                    ip: ip.clone(),
                    mac: format!("00:11:22:33:44:{:02x}", i),
                    hostname: format!("device-{:03}", i),
                    os_type: match i % 4 {
                        0 => "Ubuntu 22.04".to_string(),
                        1 => "Windows 11".to_string(),
                        2 => "macOS Sonoma".to_string(),
                        _ => "CentOS 8".to_string(),
                    },
                    status: "Online".to_string(),
                    monitoring_enabled: true,
                    username: if i % 2 == 0 { "admin".to_string() } else { "user".to_string() },
                    ..Default::default()
                };
                
                devices.push(device);
                
                if let Some(ref tx) = self.callback_tx {
                    let _ = tx.try_send(format!("Found device: {}", ip));
                }
            }
        }
        
        info!("Network scan completed. Found {} devices", devices.len());
        Ok(devices)
    }

    pub async fn scan_single_ip(&self, ip: &str) -> Result<Option<NetworkDevice>> {
        // Simulate single IP scan
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        if rand::random::<f64>() > 0.5 {
            let device = NetworkDevice {
                ip: ip.to_string(),
                mac: format!("00:11:22:33:44:{:02x}", rand::random::<u8>()),
                hostname: format!("host-{}", ip.replace('.', "-")),
                os_type: "Unknown".to_string(),
                status: "Online".to_string(),
                monitoring_enabled: false,
                ..Default::default()
            };
            Ok(Some(device))
        } else {
            Ok(None)
        }
    }
}

// Enhanced GUI with better theming and responsiveness
#[derive(Debug)]
pub struct ModernGUI {
    pub current_theme: String,
    pub font_size: f32,
    pub compact_mode: bool,
    pub last_repaint: Instant,
    pub repaint_needed: Arc<AtomicBool>,
}

impl ModernGUI {
    pub fn new() -> Self {
        Self {
            current_theme: "dark".to_string(),
            font_size: 14.0,
            compact_mode: false,
            last_repaint: Instant::now(),
            repaint_needed: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn apply_theme(&mut self, ctx: &egui::Context, theme: &str) {
        if self.current_theme == theme {
            return;
        }
        
        self.current_theme = theme.to_string();
        let mut visuals = if theme == "dark" {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        
        // Customize colors
        visuals.widgets.noninteractive.bg_fill = if theme == "dark" {
            egui::Color32::from_rgb(30, 30, 30)
        } else {
            egui::Color32::from_rgb(248, 248, 248)
        };
        
        ctx.set_visuals(visuals);
        self.mark_repaint_needed();
    }

    pub fn create_metric_card(&self, ui: &mut egui::Ui, title: &str, value: f64, unit: &str, color: egui::Color32) -> egui::Response {
        let frame = egui::Frame::default()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
            .rounding(6.0)
            .inner_margin(egui::style::Margin::same(8.0));
            
        frame.show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(title)
                    .size(self.font_size * 0.8)
                    .color(ui.visuals().weak_text_color()));
                ui.add_space(2.0);
                ui.label(egui::RichText::new(format!("{:.1}{}", value, unit))
                    .size(self.font_size * 1.4)
                    .color(color)
                    .strong());
            });
        }).response
    }

    pub fn create_status_badge(&self, ui: &mut egui::Ui, text: &str, color: egui::Color32) -> egui::Response {
        let frame = egui::Frame::default()
            .fill(color.gamma_multiply(0.1))
            .stroke(egui::Stroke::new(1.0, color))
            .rounding(12.0)
            .inner_margin(egui::style::Margin::symmetric(8.0, 4.0));
            
        frame.show(ui, |ui| {
            ui.label(egui::RichText::new(text)
                .size(self.font_size * 0.85)
                .color(color));
        }).response
    }

    pub fn mark_repaint_needed(&self) {
        self.repaint_needed.store(true, Ordering::Relaxed);
    }

    pub fn needs_repaint(&self) -> bool {
        self.repaint_needed.load(Ordering::Relaxed) || 
        self.last_repaint.elapsed() > Duration::from_secs(2)
    }
}

// Enhanced MonitoringThread with better resource management
#[derive(Debug)]
pub struct MonitoringThread {
    pub running: Arc<AtomicBool>,
    pub interval: Duration,
    pub handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    pub max_concurrent_monitors: usize,
}

impl MonitoringThread {
    pub fn new(interval_secs: u64, max_concurrent: usize) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            interval: Duration::from_secs(interval_secs),
            handle: Arc::new(RwLock::new(None)),
            max_concurrent_monitors: max_concurrent,
        }
    }

    pub async fn start(&self, 
        devices: Arc<DashMap<String, NetworkDevice>>, 
        ssh_manager: Arc<SSHManager>, 
        alert_manager: Arc<AlertManager>
    ) -> Result<()> {
        self.running.store(true, Ordering::Relaxed);
        
        let running = self.running.clone();
        let interval = self.interval;
        let max_concurrent = self.max_concurrent_monitors;
        
        let handle = tokio::spawn(async move {
            info!("Starting monitoring thread with interval: {:?}", interval);
            
            while running.load(Ordering::Relaxed) {
                let start_time = Instant::now();
                
                // Get devices that need monitoring
                let device_keys: Vec<_> = devices.iter()
                    .filter(|d| d.monitoring_enabled && d.status == "Online")
                    .take(max_concurrent)
                    .map(|d| d.key().clone())
                    .collect();
                
                if device_keys.is_empty() {
                    tokio::time::sleep(interval).await;
                    continue;
                }
                
                // Process devices in batches
                let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
                let tasks: Vec<_> = device_keys.into_iter().map(|key| {
                    let devices = devices.clone();
                    let ssh = ssh_manager.clone();
                    let alert = alert_manager.clone();
                    let semaphore = semaphore.clone();
                    
                    tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.ok()?;
                        
                        if let Some(device_ref) = devices.get_mut(&key) {
                            let result = tokio::time::timeout(
                                Duration::from_secs(30),
                                monitor_single_device(key.clone(), device_ref, ssh, alert)
                            ).await;
                            
                            match result {
                                Ok(Ok(_)) => {
                                    // Success
                                }
                                Ok(Err(e)) => {
                                    error!("Error monitoring device {}: {}", key, e);
                                }
                                Err(_) => {
                                    error!("Timeout monitoring device {}", key);
                                }
                            }
                        }
                        
                        Some(())
                    })
                }).collect();
                
                // Wait for all tasks to complete
                for task in tasks {
                    let _ = task.await;
                }
                
                let elapsed = start_time.elapsed();
                info!("Monitoring cycle completed in {:?}", elapsed);
                
                // Ensure we don't exceed the monitoring interval
                if elapsed < interval {
                    tokio::time::sleep(interval - elapsed).await;
                }
            }
            
            info!("Monitoring thread stopped");
        });
        
        let mut handle_guard = self.handle.write().await;
        *handle_guard = Some(handle);
        
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        info!("Stopping monitoring thread");
        self.running.store(false, Ordering::Relaxed);
        
        let mut handle_guard = self.handle.write().await;
        if let Some(handle) = handle_guard.take() {
            handle.abort();
            let _ = handle.await;
        }
        
        info!("Monitoring thread stopped");
        Ok(())
    }
}

// Separate function for monitoring a single device
async fn monitor_single_device(
    device_key: String,
    mut device_ref: dashmap::mapref::one::RefMut<String, NetworkDevice>,
    ssh: Arc<SSHManager>,
    alert: Arc<AlertManager>
) -> Result<()> {
    let device = device_ref.value_mut();
    
    if !ssh.connect_to_device(device).await.context("Failed to connect to device")? {
        return Ok(());
    }
    
    let metrics = ssh.get_enhanced_metrics(device).await
        .context("Failed to get metrics")?;
    let timestamp = Utc::now();
    
    // Extract metrics with better error handling
    let cpu_value = metrics["cpu"].as_f64()
        .ok_or_else(|| anyhow!("Invalid CPU metric"))?;
    let memory_value = metrics["memory"].as_f64()
        .ok_or_else(|| anyhow!("Invalid memory metric"))?;
    let disk_value = metrics["disk"].as_f64()
        .ok_or_else(|| anyhow!("Invalid disk metric"))?;
    
    // Update metrics history
    let mut history_guard = device.metrics_history.write().await;
    let metrics_to_update = [
        ("cpu", cpu_value),
        ("memory", memory_value),
        ("disk", disk_value)
    ];
    
    for (metric_name, value) in metrics_to_update {
        let history = history_guard
            .entry(metric_name.to_string())
            .or_insert_with(|| VecDeque::with_capacity(100));
        
        history.push_back((value, timestamp));
        if history.len() > 100 {
            history.pop_front();
        }
        
        // Check alerts (don't hold the lock during async operations)
        drop(history_guard);
        if let Err(e) = alert.check_metric(&device.ip, metric_name, value).await {
            error!("Failed to check alert for {} on {}: {}", metric_name, device.ip, e);
        }
        history_guard = device.metrics_history.write().await;
    }
    
    drop(history_guard);
    
    // Update top services if available
    if let Some(services_obj) = metrics.get("top_services") {
        if let Some(services_map) = services_obj.as_object() {
            for (service_name, cpu_usage) in services_map {
                if let Some(usage) = cpu_usage.as_f64() {
                    device.services.insert(service_name.clone(), usage);
                }
            }
        }
    }
    
    device.last_update = Some(timestamp);
    
    Ok(())
}

// Update event types for better state management
#[derive(Debug, Clone)]
pub enum UpdateEvent {
    DeviceUpdate { device: NetworkDevice },
    AlertUpdate { alert: Alert },
    Log { level: String, message: String },
    ScanProgress { current: usize, total: usize },
    ConfigChange { key: String, value: String },
}

// Main App struct with improved architecture
#[derive(Debug)]
pub struct EnhancedPCManagementApp {
    pub devices: Arc<DashMap<String, NetworkDevice>>,
    pub db_manager: Arc<DatabaseManager>,
    pub ssh_manager: Arc<SSHManager>,
    pub scanner: Arc<NetworkScanner>,
    pub alert_manager: Arc<AlertManager>,
    pub monitoring_thread: MonitoringThread,
    pub modern_gui: ModernGUI,
    pub config: AppConfig,
    
    // GUI state
    pub selected_device: Option<String>,
    pub search_term: String,
    pub network_range: String,
    pub alerts: Vec<Alert>,
    pub log_messages: VecDeque<String>,
    pub show_resolved_alerts: bool,
    pub show_settings: bool,
    
    // Channels for UI updates
    pub log_tx: mpsc::UnboundedSender<String>,
    pub log_rx: mpsc::UnboundedReceiver<String>,
    
    // Thread-safe flags
    pub is_scanning: Arc<AtomicBool>,
    pub is_monitoring: Arc<AtomicBool>,
    
    // Performance tracking
    pub last_update: Instant,
    pub update_count: u64,
}

impl EnhancedPCManagementApp {
    pub async fn new() -> Result<Self> {
        let config = AppConfig::default();
        let (log_tx, log_rx) = mpsc::unbounded_channel();
        
        let devices = Arc::new(DashMap::new());
        let ssh = Arc::new(SSHManager::new(&config));
        let alert_mgr = Arc::new(AlertManager::new(config.clone()));
        let scanner = Arc::new(NetworkScanner::new(log_tx.clone()));
        let monitoring = MonitoringThread::new(config.monitoring_interval, config.max_concurrent_monitors);
        let mut gui = ModernGUI::new();
        let is_scanning = Arc::new(AtomicBool::new(false));
        let is_monitoring = Arc::new(AtomicBool::new(false));

        // Load devices from database
        let db = Arc::new(DatabaseManager::new(&config.database_path, 5)
            .context("Failed to initialize database")?);
        
        match db.load_devices() {
            Ok(loaded_devices) => {
                for (key, device) in loaded_devices {
                    devices.insert(key, device);
                }
                info!("Loaded {} devices from database", devices.len());
            }
            Err(e) => {
                warn!("Failed to load devices from database: {}", e);
            }
        }

        Ok(Self {
            devices,
            db_manager: db,
            ssh_manager: ssh,
            scanner,
            alert_manager: alert_mgr,
            monitoring_thread: monitoring,
            modern_gui: gui,
            config,
            selected_device: None,
            search_term: String::new(),
            network_range: "192.168.1.0/24".to_string(),
            alerts: vec![],
            log_messages: VecDeque::new(),
            show_resolved_alerts: false,
            show_settings: false,
            log_tx,
            log_rx,
            is_scanning,
            is_monitoring,
            last_update: Instant::now(),
            update_count: 0,
        })
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        if self.is_monitoring.load(Ordering::Relaxed) {
            return Ok(());
        }
        
        self.is_monitoring.store(true, Ordering::Relaxed);
        let _ = self.log_tx.send("Starting monitoring system...".to_string());
        
        self.monitoring_thread.start(
            self.devices.clone(),
            self.ssh_manager.clone(),
            self.alert_manager.clone(),
        ).await?;
        
        let _ = self.log_tx.send("Monitoring system started successfully".to_string());
        Ok(())
    }

    pub async fn stop_monitoring(&self) -> Result<()> {
        if !self.is_monitoring.load(Ordering::Relaxed) {
            return Ok(());
        }
        
        self.is_monitoring.store(false, Ordering::Relaxed);
        let _ = self.log_tx.send("Stopping monitoring system...".to_string());
        
        self.monitoring_thread.stop().await?;
        
        let _ = self.log_tx.send("Monitoring system stopped".to_string());
        Ok(())
    }

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
        
        tokio::spawn(async move {
            let _ = log_tx.send("Starting network scan...".to_string());
            
            match scanner.scan_network(&range).await {
                Ok(found_devices) => {
                    let _ = log_tx.send(format!("Found {} devices", found_devices.len()));
                    
                    for device in found_devices {
                        devices.insert(device.mac.clone(), device.clone());
                        
                        // Save to database
                        if let Err(e) = db.save_device(&device) {
                            error!("Failed to save device {}: {}", device.ip, e);
                            let _ = log_tx.send(format!("Error saving device {}: {}", device.ip, e));
                        } else {
                            let _ = log_tx.send(format!("Saved device: {}", device.ip));
                        }
                    }
                    
                    let _ = log_tx.send("Network scan completed successfully".to_string());
                }
                Err(e) => {
                    let _ = log_tx.send(format!("Network scan failed: {}", e));
                    error!("Network scan error: {}", e);
                }
            }
            
            is_scanning_flag.store(false, Ordering::Relaxed);
        });
    }

    pub fn refresh_alerts(&mut self) {
        let active_alerts = self.alert_manager.get_active_alerts();
        
        // Only update if alerts changed
        if self.alerts.len() != active_alerts.len() || 
           self.alerts.iter().zip(&active_alerts).any(|(a, b)| a.id != b.id || a.timestamp != b.timestamp) {
            self.alerts = active_alerts;
            self.modern_gui.mark_repaint_needed();
        }
    }

    pub fn process_log_messages(&mut self) {
        let mut new_messages = 0;
        while let Ok(msg) = self.log_rx.try_recv() {
            self.log_messages.push_back(format!("[{}] {}", Utc::now().format("%H:%M:%S"), msg));
            new_messages += 1;
            
            if self.log_messages.len() > 200 {
                self.log_messages.pop_front();
            }
        }
        
        if new_messages > 0 {
            self.modern_gui.mark_repaint_needed();
        }
    }

    pub fn acknowledge_alert(&self, alert_id: &str) -> Result<()> {
        self.alert_manager.acknowledge_alert(alert_id)?;
        let _ = self.log_tx.send(format!("Alert acknowledged: {}", alert_id));
        Ok(())
    }

    fn safe_update(&mut self, ctx: &egui::Context) -> Result<()> {
        self.update_count += 1;
        
        // Process incoming log messages
        self.process_log_messages();

        // Refresh alerts periodically (every 10 updates to reduce overhead)
        if self.update_count % 10 == 0 {
            self.refresh_alerts();
        }

        // Handle keyboard shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.ctrl) {
            let new_theme = if self.modern_gui.current_theme == "light" { "dark" } else { "light" };
            self.modern_gui.apply_theme(ctx, &new_theme);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.ctrl) {
            if !self.is_scanning.load(Ordering::Relaxed) {
                self.start_network_scan();
            }
        }

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Settings").clicked() {
                        self.show_settings = !self.show_settings;
                    }
                    if ui.button("Export Data").clicked() {
                        let _ = self.log_tx.send("Export feature not implemented".to_string());
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                
                ui.menu_button("Tools", |ui| {
                    if ui.button("Network Scan (Ctrl+S)").clicked() {
                        if !self.is_scanning.load(Ordering::Relaxed) {
                            self.start_network_scan();
                        }
                    }
                    
                    let monitoring_text = if self.is_monitoring.load(Ordering::Relaxed) {
                        "Stop Monitoring"
                    } else {
                        "Start Monitoring"
                    };
                    
                    if ui.button(monitoring_text).clicked() {
                        let rt = tokio::runtime::Handle::current();
                        let app_clone = self as *const Self;
                        rt.spawn(async move {
                            // This is unsafe but necessary for the demo
                            // In a real app, you'd use proper async state management
                            let app = unsafe { &*app_clone };
                            if app.is_monitoring.load(Ordering::Relaxed) {
                                let _ = app.stop_monitoring().await;
                            } else {
                                let _ = app.start_monitoring().await;
                            }
                        });
                    }
                    
                    ui.separator();
                    if ui.button("Clear Logs").clicked() {
                        self.log_messages.clear();
                    }
                    if ui.button(format!("Toggle Resolved Alerts ({})", self.show_resolved_alerts)).clicked() {
                        self.show_resolved_alerts = !self.show_resolved_alerts;
                    }
                });
                
                ui.menu_button("View", |ui| {
                    if ui.button("Toggle Theme (Ctrl+T)").clicked() {
                        let new_theme = if self.modern_gui.current_theme == "light" { "dark" } else { "light" };
                        self.modern_gui.apply_theme(ctx, &new_theme);
                    }
                    if ui.button("Compact Mode").clicked() {
                        self.modern_gui.compact_mode = !self.modern_gui.compact_mode;
                    }
                });
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Status indicators
                    if self.is_scanning.load(Ordering::Relaxed) {
                        self.modern_gui.create_status_badge(ui, "SCANNING", egui::Color32::YELLOW);
                    }
                    if self.is_monitoring.load(Ordering::Relaxed) {
                        self.modern_gui.create_status_badge(ui, "MONITORING", egui::Color32::GREEN);
                    }
                    
                    ui.label(format!("Devices: {} | Alerts: {}", self.devices.len(), self.alerts.len()));
                });
            });
        });

        // Settings panel (if open)
        if self.show_settings {
            egui::Window::new("Settings")
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.heading("Application Settings");
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        ui.label("Theme:");
                        egui::ComboBox::from_id_source("theme")
                            .selected_text(&self.modern_gui.current_theme)
                            .show_ui(ui, |ui| {
                                if ui.selectable_value(&mut self.modern_gui.current_theme, "light".to_string(), "Light").clicked() {
                                    self.modern_gui.apply_theme(ctx, "light");
                                }
                                if ui.selectable_value(&mut self.modern_gui.current_theme, "dark".to_string(), "Dark").clicked() {
                                    self.modern_gui.apply_theme(ctx, "dark");
                                }
                            });
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Font Size:");
                        ui.add(egui::Slider::new(&mut self.modern_gui.font_size, 10.0..=24.0));
                    });
                    
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            let _ = self.log_tx.send("Settings saved".to_string());
                        }
                        if ui.button("Close").clicked() {
                            self.show_settings = false;
                        }
                    });
                });
        }

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            // Toolbar
            ui.horizontal(|ui| {
                if ui.button("🔍 Scan Network").clicked() && !self.is_scanning.load(Ordering::Relaxed) {
                    self.start_network_scan();
                }
                
                ui.separator();
                ui.label("Search:");
                ui.text_edit_singleline(&mut self.search_term);
                
                ui.separator();
                ui.label("Network:");
                ui.text_edit_singleline(&mut self.network_range);
            });

            ui.separator();

            // Main content split
            ui.horizontal(|ui| {
                // Left panel: Device list
                ui.vertical(|ui| {
                    ui.heading("Network Devices");
                    ui.horizontal(|ui| {
                        ui.label(format!("Total: {}", self.devices.len()));
                        if self.is_scanning.load(Ordering::Relaxed) {
                            ui.spinner();
                            ui.label("Scanning...");
                        }
                    });
                    ui.separator();
                    
                    egui::ScrollArea::vertical().max_height(450.0).show(ui, |ui| {
                        let filtered_devices: Vec<_> = self.devices.iter()
                            .filter(|entry| {
                                if self.search_term.is_empty() {
                                    return true;
                                }
                                let search_lower = self.search_term.to_lowercase();
                                let device = entry.value();
                                device.ip.to_lowercase().contains(&search_lower) ||
                                device.hostname.to_lowercase().contains(&search_lower) ||
                                device.os_type.to_lowercase().contains(&search_lower)
                            })
                            .collect();

                        for entry in filtered_devices {
                            let device = entry.value();
                            let is_selected = self.selected_device.as_ref()
                                .map_or(false, |mac| mac == entry.key());
                            
                            ui.horizontal(|ui| {
                                let device_label = format!("{} - {}", device.ip, device.hostname);
                                
                                if ui.selectable_label(is_selected, device_label).clicked() {
                                    self.selected_device = Some(entry.key().clone());
                                }
                                
                                // Status indicator
                                let (status_text, status_color) = match device.status.as_str() {
