use crate::core::device::LocalCommDevice;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EventPayload {
    DeviceListChanged { items: Vec<LocalCommDevice> },
}

#[derive(Serialize, Clone, Deserialize)]
pub enum ResponsePayload {
    GetDeviceList { devices: Vec<LocalCommDevice> },
}

#[derive(Serialize, Clone, Deserialize)]
pub enum RequestPayload {}
