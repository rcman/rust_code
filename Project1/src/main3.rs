// Key improvements to consider:

// 1. Better error handling in GUI updates
impl eframe::App for EnhancedPCManagementApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Wrap potentially failing operations in error handling
        if let Err(e) = self.safe_update(ctx) {
            // Log error instead of panicking
            error!("GUI update error: {}", e);
        }
        
        // Only repaint when needed
        if self.needs_repaint() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }
}

// 2. Improved monitoring thread without runtime conflicts
impl MonitoringThread {
    pub fn start_async(&self, 
        devices: Arc<DashMap<String, NetworkDevice>>, 
        ssh_manager: Arc<SSHManager>, 
        alert_manager: Arc<AlertManager>
    ) -> tokio::task::JoinHandle<()> {
        let running = self.running.clone();
        let interval = self.interval;
        
        tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                // Process devices in batches to avoid overwhelming the system
                let device_keys: Vec<_> = devices.iter()
                    .filter(|d| d.monitoring_enabled)
                    .take(10) // Process max 10 devices at once
                    .map(|d| d.key().clone())
                    .collect();
                
                let tasks: Vec<_> = device_keys.into_iter().map(|key| {
                    let devices = devices.clone();
                    let ssh = ssh_manager.clone();
                    let alert = alert_manager.clone();
                    
                    tokio::spawn(async move {
                        if let Some(mut device_ref) = devices.get_mut(&key) {
                            // Use a timeout to prevent hanging
                            let result = tokio::time::timeout(
                                Duration::from_secs(30),
                                monitor_device(device_ref.value_mut(), &ssh, &alert)
                            ).await;
                            
                            if let Err(_) = result {
                                error!("Device monitoring timeout for {}", key);
                            }
                        }
                    })
                }).collect();
                
                // Wait for all tasks to complete or timeout
                for task in tasks {
                    let _ = task.await;
                }
                
                tokio::time::sleep(interval).await;
            }
        })
    }
}

// 3. Separate monitoring function for better error handling
async fn monitor_device(
    device: &mut NetworkDevice,
    ssh: &SSHManager,
    alert: &AlertManager
) -> Result<()> {
    if !ssh.connect_to_device(device).await? {
        return Ok(());
    }
    
    let metrics = ssh.get_enhanced_metrics(device).await?;
    let timestamp = Utc::now();
    
    // Extract metrics with better error handling
    let cpu_value = metrics["cpu"].as_f64().ok_or_else(|| {
        anyhow::anyhow!("Invalid CPU metric")
    })?;
    
    let memory_value = metrics["memory"].as_f64().ok_or_else(|| {
        anyhow::anyhow!("Invalid memory metric")
    })?;
    
    let disk_value = metrics["disk"].as_f64().ok_or_else(|| {
        anyhow::anyhow!("Invalid disk metric")
    })?;
    
    // Update metrics in a single batch
    let metrics_to_update = [
        ("cpu", cpu_value),
        ("memory", memory_value),
        ("disk", disk_value)
    ];
    
    for (metric_name, value) in metrics_to_update {
        let mut history = device.metrics_history
            .entry(metric_name.to_string())
            .or_insert_with(|| VecDeque::with_capacity(100));
        
        history.push_back((value, timestamp));
        if history.len() > 100 {
            history.pop_front();
        }
        
        // Check alerts
        alert.check_metric(&device.ip, metric_name, value);
    }
    
    device.last_update = Some(timestamp);
    Ok(())
}

// 4. Better database connection management
pub struct DatabaseManager {
    pub pool: Arc<Mutex<Vec<Connection>>>,
    pub db_path: String,
    pub max_connections: usize,
}

impl DatabaseManager {
    pub fn new(db_path: &str) -> Result<Self> {
        let mut connections = Vec::new();
        let max_connections = 5;
        
        // Create connection pool
        for _ in 0..max_connections {
            let conn = Connection::open(db_path)?;
            conn.execute("PRAGMA journal_mode=WAL;", [])?;
            connections.push(conn);
        }
        
        let mut db = Self {
            pool: Arc::new(Mutex::new(connections)),
            db_path: db_path.to_string(),
            max_connections,
        };
        
        // Initialize tables with first connection
        db.init_database()?;
        Ok(db)
    }
    
    pub fn with_connection<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R>,
    {
        let mut pool = self.pool.lock().unwrap();
        if let Some(conn) = pool.pop() {
            let result = f(&conn);
            pool.push(conn);
            result
        } else {
            // All connections busy, create temporary one
            let conn = Connection::open(&self.db_path)?;
            f(&conn)
        }
    }
}

// 5. Improved alert deduplication
impl AlertManager {
    pub fn check_metric(&self, device_ip: &str, metric: &str, value: f64) {
        if let Some(threshold) = self.thresholds.get(metric) {
            if !threshold.enabled {
                return;
            }
            
            self.anomaly_detector.update_baseline(device_ip, metric, value);
            let alert_key = format!("{}_{}", device_ip, metric);
            
            // Check current alert status
            let current_alert = self.alerts.get(&alert_key);
            
            let new_level = if self.anomaly_detector.detect_anomaly(device_ip, metric, value) {
                Some("anomaly")
            } else if value >= threshold.critical_level {
                Some("critical")
            } else if value >= threshold.warning_level {
                Some("warning")
            } else {
                None
            };
            
            match (current_alert, new_level) {
                (Some(mut alert), Some(level)) => {
                    // Update existing alert if level changed or not resolved
                    if alert.level != level || alert.resolved {
                        alert.level = level.to_string();
                        alert.value = value;
                        alert.timestamp = Utc::now();
                        alert.resolved = false;
                    }
                }
                (Some(mut alert), None) => {
                    // Resolve existing alert
                    alert.resolved = true;
                }
                (None, Some(level)) => {
                    // Create new alert
                    self.create_alert(device_ip, metric, value, level.to_string(), threshold.warning_level, &alert_key);
                }
                (None, None) => {
                    // No action needed
                }
            }
        }
    }
}

// 6. Resource cleanup
impl Drop for EnhancedPCManagementApp {
    fn drop(&mut self) {
        // Clean shutdown
        self.monitoring_thread.running.store(false, Ordering::Relaxed);
        
        // Save all devices to database
        for device_entry in self.devices.iter() {
            if let Err(e) = self.db_manager.save_device(device_entry.value()) {
                error!("Failed to save device on shutdown: {}", e);
            }
        }
        
        info!("Application cleanup completed");
    }
}
