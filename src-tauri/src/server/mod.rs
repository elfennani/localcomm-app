use crate::core::device::{LocalCommDevice, SharedLocalCommDeviceList};
use std::fs;

use crate::localcomm::local_comm_server::{LocalComm, LocalCommServer};
use crate::localcomm::{
    Device, Empty, GetDeviceListRequest, GetDeviceListResponse, RunCommandRequest, SendFileRequest,
    TextTypeRequest,
};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use indicatif::{ProgressBar, ProgressStyle};
use local_ip_address::local_ip;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::core::cert_gen::{gen_csr, gen_server_cert_key, sign_csr};
use crate::core::{ROOT_CA_CERT, ROOT_CA_KEY};
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio::sync::watch::Receiver;
use tokio_stream::wrappers::ReceiverStream;
use tonic::codegen::tokio_stream::Stream;
use tonic::transport::server::TcpIncoming;
use tonic::transport::Server;
use tonic::{Request, Response, Status, Streaming};
use tonic_tls::rustls::TlsIncoming;

impl From<LocalCommDevice> for Device {
    fn from(device: LocalCommDevice) -> Self {
        Device {
            name: device.name,
            address: device.address,
            resolved_host: device.resolved_host,
        }
    }
}

impl From<Device> for LocalCommDevice {
    fn from(device: Device) -> Self {
        LocalCommDevice {
            name: device.name,
            address: device.address,
            resolved_host: device.resolved_host,
        }
    }
}

#[derive(Debug)]
pub struct LocalCommServerApp {
    device_list: SharedLocalCommDeviceList,
    device_list_rx: Receiver<Vec<LocalCommDevice>>,
    progress_bar: Arc<Mutex<Option<ProgressBar>>>,
    download_dir: PathBuf,
    app_data_dir: PathBuf,
    uploading_file: Arc<Mutex<Option<File>>>,
}

impl LocalCommServerApp {
    pub async fn serve(
        rx: Receiver<Vec<LocalCommDevice>>,
        devices: SharedLocalCommDeviceList,
        download_dir: PathBuf,
        app_data_dir: PathBuf,
    ) -> Result<(), Box<dyn Error>> {
        // let addr = "0.0.0.0:50051".parse()?;
        let localcomm =
            LocalCommServerApp::new(rx, devices.clone(), download_dir, app_data_dir.clone());
        let ip = local_ip().unwrap();

        println!("LocalComm instance listening on {}:50051", ip);
        let server = Server::builder()
            .add_service(LocalCommServer::new(localcomm))
            .serve_with_incoming(Self::generate_certs(app_data_dir.join("certs")));

        Ok(server.await.unwrap())
    }

    fn generate_certs(certs_dir: PathBuf) -> TlsIncoming<TcpStream> {
        if !certs_dir.exists() {
            fs::create_dir(&certs_dir).unwrap();
        }

        let private_key_file = certs_dir.join("server.key");

        if !private_key_file.exists() {
            gen_server_cert_key(&certs_dir).unwrap();
        }

        let cert_file = certs_dir.join("server.crt");
        if !cert_file.exists() {
            gen_csr(&certs_dir, None).unwrap();

            let root_ca_key = str::from_utf8(ROOT_CA_KEY).unwrap();
            let root_ca_cert = str::from_utf8(ROOT_CA_CERT).unwrap();

            sign_csr(
                &certs_dir,
                root_ca_cert.to_string(),
                root_ca_key.to_string(),
            )
            .unwrap();
        }

        let certs = CertificateDer::pem_file_iter(cert_file)
            .unwrap()
            .map(|cert| cert.unwrap())
            .collect();
        let private_key = PrivateKeyDer::from_pem_file(private_key_file).unwrap();

        let config = tokio_rustls::rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, private_key)
            .unwrap();

        let addr = "0.0.0.0:50051".parse().unwrap();
        let inc = TlsIncoming::new(TcpIncoming::bind(addr).unwrap(), Arc::new(config));

        inc
    }

    pub fn new(
        rx: Receiver<Vec<LocalCommDevice>>,
        device_list: SharedLocalCommDeviceList,
        download_dir: PathBuf,
        app_data_dir: PathBuf,
    ) -> Self {
        // let user_dirs = directories::UserDirs::new().expect("cannot get user directories");
        // let download_dir = user_dirs
        //     .download_dir()
        //     .expect("Failed to retrieve download directory");

        LocalCommServerApp {
            device_list_rx: rx,
            device_list,
            progress_bar: Arc::new(Mutex::new(None)),
            uploading_file: Arc::new(Mutex::new(None)),
            download_dir,
            app_data_dir,
        }
    }

    pub fn unique_path(parent: PathBuf, name: String) -> PathBuf {
        let mut base = parent.clone();
        base.push(&name);

        if !base.exists() {
            return base;
        }

        let path = Path::new(&name);
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ext = path.extension().and_then(|e| e.to_str());

        for i in 1.. {
            let candidate_name = if let Some(ext) = ext {
                format!("{stem} ({i}).{ext}")
            } else {
                format!("{stem} ({i})")
            };

            let mut candidate = parent.clone();
            candidate.push(&candidate_name);

            if !candidate.exists() {
                return candidate;
            }
        }

        unreachable!()
    }
}

type DeviceListResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<GetDeviceListResponse, Status>> + Send>>;

#[tonic::async_trait]
impl LocalComm for LocalCommServerApp {
    async fn get_device_list(
        &self,
        request: Request<GetDeviceListRequest>,
    ) -> Result<Response<GetDeviceListResponse>, Status> {
        println!("Got a request from {:?}", request.remote_addr());
        let device_list: Vec<Device> = self
            .device_list
            .lock()
            .unwrap()
            .iter()
            .map(|d| Device {
                name: d.name.clone(),
                address: d.address.clone(),
                resolved_host: d.resolved_host.clone(),
            })
            .collect();

        Ok(Response::new(GetDeviceListResponse { list: device_list }))
    }

    async fn type_text(
        &self,
        request: Request<TextTypeRequest>,
    ) -> Result<Response<Empty>, Status> {
        let mut enigo =
            Enigo::new(&Settings::default()).map_err(|e| Status::unknown(e.to_string()))?;

        let req = request.into_inner();
        let text = req.text;

        enigo
            .text(text.as_str())
            .map_err(|e| Status::unknown(e.to_string()))
            .unwrap_or_default();

        if req.submit {
            enigo.key(Key::Return, Direction::Click).unwrap_or_default();
        }

        Ok(Response::new(Empty {}))
    }

    async fn run_command(
        &self,
        request: Request<RunCommandRequest>,
    ) -> Result<Response<Empty>, Status> {
        Command::new("sh")
            .arg("-c")
            .arg(request.into_inner().command)
            .output()
            .expect("failed to execute");

        Ok(Response::new(Empty {}))
    }

    async fn send_file(
        &self,
        request: tonic::Request<Streaming<SendFileRequest>>,
    ) -> Result<Response<Empty>, Status> {
        let mut stream = request.into_inner();

        while let Some(req) = stream.message().await? {
            let mut progress_bar = self.progress_bar.lock().unwrap();
            let mut file = self.uploading_file.lock().unwrap();

            if req.position == 0 {
                *progress_bar = Some(
                    ProgressBar::new(req.size)
                        .with_style(
                            ProgressStyle::default_bar()
                                .template(
                                    "{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
                                )
                                .unwrap(),
                        )
                        .with_message(format!("Saving {}", req.name)),
                );
                *file = Some(
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(Self::unique_path(
                            self.download_dir.clone(),
                            req.name.clone(),
                        ))
                        .expect("cannot open file"),
                );
                println!(
                    "Got a request to receive a file {} ({} bytes)",
                    req.name, req.size
                )
            };

            if let Some(f) = file.as_mut() {
                f.write_all(req.bytes.as_slice())
                    .expect("Failed to write file");
            }

            if let Some(progress_bar) = &*progress_bar {
                progress_bar.set_position(req.position);
            }

            if req.size - req.position <= (req.buffer_size as u64) {
                if let Some(progress_bar) = &*progress_bar {
                    progress_bar.finish_with_message("Done");
                }

                if let Some(f) = file.as_mut() {
                    println!("Saved File in {}", self.download_dir.display());
                    f.flush().expect("Failed to write file");
                    *file = None;
                }
            }
        }

        Ok(Response::new(Empty {}))
    }

    type ListenForDevicesStream = ResponseStream;

    async fn listen_for_devices(
        &self,
        request: Request<Empty>,
    ) -> DeviceListResult<Self::ListenForDevicesStream> {
        let (tx, rx) = tokio::sync::mpsc::channel(128);
        let mut device_list_rx = self.device_list_rx.clone();

        tokio::spawn(async move {
            loop {
                let device_list = device_list_rx.borrow().clone();
                let response = GetDeviceListResponse {
                    list: device_list.into_iter().map(Device::from).collect(),
                };
                match tx.send(Result::<_, Status>::Ok(response)).await {
                    Err(_item) => {
                        break;
                    }
                    _ => {}
                }

                if device_list_rx.changed().await.is_err() {
                    break;
                }
            }

            println!("\tclient disconnected");
        });

        let output_stream = ReceiverStream::new(rx);

        Ok(Response::new(
            Box::pin(output_stream) as Self::ListenForDevicesStream
        ))
    }
}
