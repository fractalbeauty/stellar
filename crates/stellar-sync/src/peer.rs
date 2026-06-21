use iroh::{
    Endpoint, EndpointId, SecretKey,
    protocol::{ProtocolHandler, Router},
};

#[derive(Debug)]
pub struct Peer {
    router: Router,
}

impl Peer {
    pub async fn start() -> Result<Self, anyhow::Error> {
        let secret_key = SecretKey::generate();

        let endpoint = Endpoint::builder(iroh::endpoint::presets::N0)
            .secret_key(secret_key)
            .bind()
            .await?;

        let router = iroh::protocol::Router::builder(endpoint)
            .accept(Protocol::ALPN, Protocol)
            .spawn();

        Ok(Self { router })
    }

    pub fn endpoint_id(&self) -> EndpointId {
        self.router.endpoint().id()
    }
}

#[derive(Debug)]
struct Protocol;

impl ProtocolHandler for Protocol {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        todo!()
    }
}

impl Protocol {
    const ALPN: &'static str = "stellar-sync/1";
}
