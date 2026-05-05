use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct LocalCommDevice {
    pub name: String,
    pub address: String,
}

pub type SharedLocalCommDeviceList = Arc<Mutex<Vec<LocalCommDevice>>>;
