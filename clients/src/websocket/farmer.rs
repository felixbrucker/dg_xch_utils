use crate::websocket::{
    get_client, get_client_generated_tls, get_client_tls, perform_handshake, Client,
    ClientSSLConfig, NodeType,
};
use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct FarmerClient {
    pub client: Arc<Mutex<Client>>,
    handle: JoinHandle<()>,
}
impl FarmerClient {
    pub async fn new_ssl_generate(
        host: &str,
        port: u16,
        network_id: &str,
        additional_headers: &Option<HashMap<String, String>>,
        run: Arc<AtomicBool>,
    ) -> Result<Self, Error> {
        let (client, mut stream) = get_client_generated_tls(host, port, additional_headers).await?;
        let handle = tokio::spawn(async move { stream.run(run).await });
        let client = Arc::new(Mutex::new(client));
        perform_handshake(client.clone(), network_id, port, NodeType::Farmer).await?;
        Ok(FarmerClient { client, handle })
    }
    pub async fn new_ssl(
        host: &str,
        port: u16,
        ssl_info: ClientSSLConfig<'_>,
        network_id: &str,
        additional_headers: &Option<HashMap<String, String>>,
        run: Arc<AtomicBool>,
    ) -> Result<Self, Error> {
        let (client, mut stream) = get_client_tls(host, port, ssl_info, additional_headers).await?;
        let handle = tokio::spawn(async move { stream.run(run).await });
        let client = Arc::new(Mutex::new(client));
        perform_handshake(client.clone(), network_id, port, NodeType::Farmer).await?;
        Ok(FarmerClient { client, handle })
    }
    pub async fn new(
        host: &str,
        port: u16,
        network_id: &str,
        additional_headers: &Option<HashMap<String, String>>,
        run: Arc<AtomicBool>,
    ) -> Result<Self, Error> {
        let (client, mut stream) = get_client(host, port, additional_headers).await?;
        let handle = tokio::spawn(async move { stream.run(run).await });
        let client = Arc::new(Mutex::new(client));
        perform_handshake(client.clone(), network_id, port, NodeType::Farmer).await?;
        Ok(FarmerClient { client, handle })
    }

    pub async fn join(self) -> Result<(), Error> {
        self.handle
            .await
            .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to join farmer: {:?}", e)))?;
        self.client.lock().await.shutdown().await
    }

    pub fn is_closed(&self) -> bool {
        self.handle.is_finished()
    }
}
