use iroh::endpoint::{RecvStream, SendStream};
use serde::{Deserialize, Serialize};
use tokio_util::{
    bytes::Bytes,
    codec::{FramedRead, FramedWrite, LengthDelimitedCodec},
};

#[derive(Debug, Serialize, Deserialize)]
pub enum StreamHeader {
    Sync,
    Difference,
    SchemaSync,
}

impl StreamHeader {
    pub fn encode(message: &Self) -> Bytes {
        postcard::to_stdvec(&message)
            .expect("Failed to serialize message")
            .into()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        postcard::from_bytes(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize message: {e:?}"))
    }
}

fn with_codecs(
    tx: SendStream,
    rx: RecvStream,
) -> (
    FramedWrite<SendStream, LengthDelimitedCodec>,
    FramedRead<RecvStream, LengthDelimitedCodec>,
) {
    let tx = FramedWrite::new(tx, LengthDelimitedCodec::new());
    let rx = FramedRead::new(rx, LengthDelimitedCodec::new());

    (tx, rx)
}
