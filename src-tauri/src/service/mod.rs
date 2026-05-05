use crate::core::device::{LocalCommDevice, SharedLocalCommDeviceList};
use local_ip_address::local_ip;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use slugify::slugify;
use std::sync::{Arc, Mutex};
use tokio::task;

pub struct LocalCommService {
    service_type: String,
    mdns: Arc<ServiceDaemon>,
    pub devices: SharedLocalCommDeviceList,
}

impl LocalCommService {
    pub fn new(service_type: &str) -> Self {
        let mdns = Arc::new(ServiceDaemon::new().expect("Failed to create daemon"));

        LocalCommService {
            service_type: service_type.to_string(),
            mdns,
            devices: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn start(&mut self) {
        self.broadcast_service();
        self.start_discovery();
    }

    pub fn stop(&self) {
        // Gracefully shutdown the daemon.
        println!("Stopping service {}", self.service_type);
        self.mdns.shutdown().unwrap();
    }

    fn broadcast_service(&self) {
        let service_type = self.service_type.clone();
        let mdns = self.mdns.clone();

        task::spawn(async move {
            println!("Broadcast service started");

            let receiver = mdns.monitor().expect("Failed to monitor daemon");
            task::spawn(async move {
                while let Ok(event) = receiver.recv() {
                    match event {
                        mdns_sd::DaemonEvent::Error(error) => {
                            eprintln!("[SERVICE_BROADCAST] Daemon error: {error}");
                        }
                        event => {
                            println!("[SERVICE_BROADCAST] {event:?}");
                        }
                    }
                }
            });

            let device_name = whoami::hostname().unwrap();
            let instance_name = slugify!(&device_name, separator = "_");
            let ip = local_ip().unwrap().to_string();
            let host_name = format!("{}.local.", instance_name);
            let port = 5200;
            let properties = [("device_name", &instance_name)];

            println!(
                "[SERVICE_BROADCAST] Broadcasting mDNS service ({}) on {}:{} with host name {}",
                service_type, ip, port, host_name
            );

            let my_service = ServiceInfo::new(
                &service_type,
                &instance_name,
                &host_name,
                ip,
                port,
                &properties[..],
            )
            .unwrap();

            mdns.register(my_service)
                .expect("Failed to register our service");
        });
    }

    fn start_discovery(&mut self) {
        let mdns = self.mdns.clone();

        let service_type = self.service_type.clone();
        let device_name = slugify!(whoami::hostname().unwrap().as_str(), separator = "_");
        let receiver = mdns.browse(&service_type).expect("Failed to browse");
        let device_list_mutex = self.devices.clone();

        task::spawn(async move {
            println!("[SERVICE_DISCOVERY] Discovery started");
            while let Ok(event) = receiver.recv() {
                match event {
                    ServiceEvent::ServiceResolved(resolved) => {
                        if let Some(prop) = resolved.txt_properties.get("device_name") {
                            if prop.val_str() == device_name {
                                continue;
                            }
                        }

                        let ip_addr = match resolved.addresses.iter().find(|d| d.is_ipv4()) {
                            None => {
                                continue;
                            }
                            Some(ip) => ip.to_string(),
                        };

                        let device_name = match resolved.txt_properties.get("device_name") {
                            None => "Not named",
                            Some(property) => property.val_str(),
                        };
                        println!(
                            "[SERVICE_DISCOVERY] Resolved a new service: {} ({})",
                            resolved.fullname, device_name
                        );

                        println!("[SERVICE_DISCOVERY] Service resolved: {:?}", resolved,);

                        let mut lock = device_list_mutex.lock().unwrap();

                        if lock.iter().all(|d| d.name != device_name) {
                            (*lock).push(LocalCommDevice {
                                name: device_name.to_string(),
                                address: format!("http://{}:50051", ip_addr),
                            });
                        }
                    }
                    ServiceEvent::ServiceFound(_, full_name) => {
                        if full_name.starts_with(device_name.as_str()) {
                            continue;
                        }

                        println!("[SERVICE_DISCOVERY] Service found: {}", full_name);
                    }
                    ServiceEvent::SearchStarted(_) => {}
                    other_event => {
                        println!(
                            "[SERVICE_DISCOVERY] Received other event: {:?}",
                            &other_event
                        );
                    }
                }
            }
        });
    }
}
