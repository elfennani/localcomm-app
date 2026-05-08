use crate::core::device::LocalCommDevice;
use crate::localcomm::local_comm_client::LocalCommClient;
use crate::localcomm::{Empty, GetDeviceListRequest};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName, TrustAnchor};
use std::env;
use std::sync::Arc;
use tokio::sync::watch;
use tokio_rustls::rustls::RootCertStore;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tonic::transport::Channel;
use tonic::Request;
use crate::core::ROOT_CA_CERT;

/// An abstract client to communicate with the same device's localcomm server
pub struct LocalCommAppClient {
    url: String,
    client: LocalCommClient<Channel>,
}

impl LocalCommAppClient {
    pub async fn connect(url: &str) -> Self {
        let ssl_conn = Self::make_ssl_conn();
        // Connect to remote
        let ch = Self::connect_tonic_channel(ssl_conn).await.unwrap();

        Self {
            url: url.to_string(),
            client: LocalCommClient::new(ch),
        }
    }

    async fn connect_tonic_channel(
        ssl_conn: Arc<tokio_rustls::rustls::ClientConfig>,
    ) -> Result<tonic::transport::Channel, tonic_tls::Error> {
        let ep = tonic::transport::Endpoint::from_static("https://localhost:50051");
        let transport = tonic_tls::TcpTransport::from_endpoint(&ep);
        let server_name: ServerName = "localhost".try_into()?;
        Ok(ep
            .connect_with_connector(tonic_tls::rustls::TlsConnector::new(
                transport,
                ssl_conn,
                server_name, // server has cert with dns localhost
            ))
            .await
            .unwrap())
    }

    fn make_ssl_conn() -> Arc<tokio_rustls::rustls::ClientConfig> {
        let mut root_cert_store = RootCertStore::empty();

        let certs = CertificateDer::pem_slice_iter(ROOT_CA_CERT)
            .map(|cert| cert.unwrap());

        root_cert_store.add_parsable_certificates(certs);

        let config = tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();

        Arc::new(config)
    }
    pub async fn device_listener(
        &mut self,
        tx: watch::Sender<Vec<LocalCommDevice>>,
        cancel_token: Option<CancellationToken>,
    ) {
        let mut stream = self
            .client
            .listen_for_devices(Empty {})
            .await
            .unwrap()
            .into_inner();

        loop {
            tokio::select! {
                _ = async {
                    if let Some(cancel_token) = cancel_token.clone() {
                        cancel_token.cancelled().await;
                    }else {
                        std::future::pending::<()>().await;
                    }
                } => (),
                item = stream.next() => {
                    if let Some(item) = item {
                        let list = item.unwrap().list;
                        let list: Vec<LocalCommDevice> = list.into_iter().map(LocalCommDevice::from).collect();

                        tx.send(list).ok();
                    }else{
                        break;
                    }
                }
            }
        }
    }
}

async fn create_device_client(
    local_client: &mut LocalCommClient<Channel>,
    device_name: &str,
) -> LocalCommClient<Channel> {
    let request = Request::new(GetDeviceListRequest {});
    let response = local_client.get_device_list(request).await.unwrap();
    let address = response
        .into_inner()
        .list
        .iter()
        .find(|d| d.name == *device_name)
        .expect("Device not found!")
        .address
        .clone();

    LocalCommClient::connect(address).await.unwrap()
}
