use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default, Serialize, Clone, Deserialize)]
pub struct LocalCommDevice {
    pub name: String,
    pub address: String,
    pub resolved_host: String,
}

pub type SharedLocalCommDeviceList = Arc<Mutex<Vec<LocalCommDevice>>>;
