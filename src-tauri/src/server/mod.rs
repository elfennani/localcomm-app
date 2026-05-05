use crate::core::device::SharedLocalCommDeviceList;
use crate::server::localcomm::{
    Device, Empty, GetDeviceListRequest, GetDeviceListResponse, RunCommandRequest, SendFileRequest,
    TextTypeRequest,
};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use indicatif::{ProgressBar, ProgressStyle};
use localcomm::local_comm_server::{LocalComm, LocalCommServer};
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use local_ip_address::local_ip;
use tonic::transport::Server;
use tonic::{Request, Response, Status, Streaming};

pub mod localcomm {
    tonic::include_proto!("localcomm");
}

#[derive(Debug)]
pub struct LocalCommServerApp {
    device_list: SharedLocalCommDeviceList,
    progress_bar: Arc<Mutex<Option<ProgressBar>>>,
    download_dir: PathBuf,
    uploading_file: Arc<Mutex<Option<File>>>,
}

impl LocalCommServerApp {
    pub async fn serve(
        devices: SharedLocalCommDeviceList,
        download_dir: PathBuf,
    ) -> Result<(), Box<dyn Error>> {
        let addr = "0.0.0.0:50051".parse()?;
        let localcomm = LocalCommServerApp::new(devices.clone(), download_dir);
        let ip = local_ip().unwrap();

        println!("LocalComm instance listening on {}:50051", ip);
        let server = Server::builder()
            .add_service(LocalCommServer::new(localcomm))
            .serve(addr);

        Ok(server.await?)
    }

    pub fn new(device_list: SharedLocalCommDeviceList, download_dir: PathBuf) -> Self {
        // let user_dirs = directories::UserDirs::new().expect("cannot get user directories");
        // let download_dir = user_dirs
        //     .download_dir()
        //     .expect("Failed to retrieve download directory");

        LocalCommServerApp {
            device_list,
            progress_bar: Arc::new(Mutex::new(None)),
            uploading_file: Arc::new(Mutex::new(None)),
            download_dir,
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
}
