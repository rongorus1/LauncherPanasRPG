use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct DeviceDiagnostics {
    pub total_memory_gb: Option<f64>,
    pub processor_name: Option<String>,
    pub logical_cores: Option<usize>,
    pub gpu_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanReport {
    pub installed_version: Option<String>,
    pub checked_files: usize,
    pub missing_files: Vec<String>,
    pub corrupt_archives: Vec<String>,
    pub unchecked_files: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub version: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreReport {
    pub restored_files: usize,
    pub version: String,
}
