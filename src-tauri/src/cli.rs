use crate::core::device::LocalCommDevice;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::path::Path;
use tokio::sync::watch;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;

pub mod localcomm {
    tonic::include_proto!("localcomm");
}

mod core;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    ListDevices,
    Type {
        #[arg(short, long)]
        text: String,
        #[arg(short, long)]
        device: String,
        #[arg(short, long)]
        submit: bool,
    },
    RunCommand {
        #[arg(short, long)]
        device: String,
        #[arg(short, long)]
        command: String,
    },
    SendFile {
        #[arg(short, long)]
        device: String,
        #[arg(short, long)]
        path: String,
        #[arg(short, long)]
        buffer: Option<u32>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // let cli = Cli::parse();

    // let mut client = LocalCommAppClient::connect("https://0.0.0.0:50051").await;
    // let (tx, mut rx) = watch::channel(Vec::<LocalCommDevice>::new());
    //
    // tokio::spawn(async move {
    //     client.device_listener(tx, None).await;
    // });
    //
    // loop {
    //     let list = rx.borrow().clone();
    //
    //     println!("Device list changed!");
    //     list.iter().for_each(|device| {
    //         println!("{:?}", device);
    //     });
    //
    //     if rx.changed().await.is_err() {
    //         break;
    //     }
    // }

    // match &cli.command {
    //     Some(Commands::Type {
    //              text,
    //              device: device_name,
    //              submit,
    //          }) => {
    //         let mut client = create_device_client(&mut client, device_name.as_str()).await;
    //         let request = Request::new(TextTypeRequest {
    //             text: text.clone(),
    //             submit: *submit,
    //         });
    //         client.type_text(request).await?;
    //     }
    //     Some(Commands::ListDevices) => {
    //         let request = Request::new(GetDeviceListRequest {});
    //         let response = client.get_device_list(request).await?;
    //
    //         response.into_inner().list.iter().for_each(|d| {
    //             println!("{}: {}", d.name, d.address);
    //         });
    //     }
    //     Some(Commands::RunCommand { device, command }) => {
    //         let mut client = create_device_client(&mut client, device.as_str()).await;
    //         let request = Request::new(RunCommandRequest {
    //             command: command.to_string(),
    //         });
    //         client.run_command(request).await?;
    //     }
    //     Some(Commands::SendFile {
    //              device,
    //              path,
    //              buffer,
    //          }) => {
    //         let mut client = create_device_client(&mut client, device.as_str()).await;
    //         let (tx, rx) = tokio::sync::mpsc::channel(32);
    //
    //         let buffer = buffer.clone();
    //         let path: String = path.clone();
    //
    //         let file_name = path.split("/").last().unwrap().to_string();
    //         let path = Path::new(path.as_str());
    //         let mut file = File::open(path).expect("Failed to open file");
    //         let mut written: u64 = 0;
    //         let size = std::fs::metadata(path)
    //             .expect("Failed to read metadata")
    //             .len();
    //         let buffer_size: usize = buffer.unwrap_or((128 * 1024) as u32) as usize;
    //         let progress_bar = ProgressBar::new(size)
    //             .with_style(
    //                 ProgressStyle::default_bar()
    //                     .template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
    //                     .unwrap(),
    //             )
    //             .with_message(format!("Sending {}", file_name));
    //
    //         if path.is_dir() {
    //             panic!("Path is a directory");
    //         }
    //
    //         tokio::spawn(async move {
    //             loop {
    //                 let mut buffer = vec![0u8; buffer_size];
    //                 let n = file.read(&mut buffer[..]).unwrap();
    //
    //                 if n == 0 {
    //                     break;
    //                 }
    //
    //                 tx.send(SendFileRequest {
    //                     name: file_name.to_string(),
    //                     position: written,
    //                     bytes: buffer[..n].to_vec(),
    //                     size,
    //                     buffer_size: 128 * 1024,
    //                 })
    //                     .await
    //                     .unwrap();
    //
    //                 written += n as u64;
    //                 progress_bar.set_position(written)
    //             }
    //
    //             progress_bar.finish_with_message(format!("{} sent!", file_name));
    //         });
    //
    //         let stream = ReceiverStream::new(rx);
    //         client.send_file(stream).await?;
    //     }
    //
    //     None => {}
    // };

    Ok(())
}
