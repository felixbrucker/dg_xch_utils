use crate::clients::websocket::{get_client, get_client_tls, perform_handshake, Client, NodeType};
use std::io::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::{JoinError, JoinHandle};

pub struct HarvesterClient {
    pub client: Arc<Mutex<Client>>,
    handle: JoinHandle<()>,
}
impl HarvesterClient {
    pub async fn new(host: &str, port: u16, network_id: &str) -> Result<Self, Error> {
        let (client, mut stream) = get_client(host, port).await?;
        let handle = tokio::spawn(async move { stream.run().await });
        let client = Arc::new(Mutex::new(client));
        perform_handshake(client.clone(), network_id, port, NodeType::Harvester).await?;
        Ok(HarvesterClient { client, handle })
    }
    pub async fn new_ssl(
        host: &str,
        port: u16,
        ssl_crt_path: &str,
        ssl_key_path: &str,
        ssl_ca_crt_path: &str,
        network_id: &str,
    ) -> Result<Self, Error> {
        let (client, mut stream) =
            get_client_tls(host, port, ssl_crt_path, ssl_key_path, ssl_ca_crt_path).await?;
        let handle = tokio::spawn(async move { stream.run().await });
        let client = Arc::new(Mutex::new(client));
        perform_handshake(client.clone(), network_id, port, NodeType::Harvester).await?;
        Ok(HarvesterClient { client, handle })
    }

    pub async fn join(self) -> Result<(), JoinError> {
        self.handle.await
    }
}
