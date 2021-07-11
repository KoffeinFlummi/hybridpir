use std::io::{Error, ErrorKind, Read, Write};

use bitvec::prelude::*;
use sealpir::{PirQuery, PirReply};
use serde::{Serialize, Deserialize};
use bincode;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum HybridPirMessage {
    Hello,
    Seed(u128),
    Query(
        #[serde(with = "serde_bytes")]
        Vec<u8>,
        #[serde(with = "serde_bytes")]
        Vec<u8>,
        PirQuery
    ),
    Response(PirReply),
}

impl HybridPirMessage {
    /**
     * Write message to target, serializing it to bincode.
     *
     * ```
     * use hybridpir::types::HybridPirMessage;
     *
     * let mut buffer: Vec<u8> = Vec::new();
     * let mut cursor = std::io::Cursor::new(buffer);
     *
     * let message = HybridPirMessage::Seed(1234);
     * message.write_to(&mut cursor).unwrap();
     * ```
     */
    pub fn write_to<W: Write>(&self, mut stream: &mut W) -> Result<(), std::io::Error> {
        bincode::serialize_into(&mut stream, self)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{}", e)))
    }

    /**
     * Read message from stream and deserialize.
     *
     * ```
     * use std::io::{Seek, SeekFrom};
     * use hybridpir::types::HybridPirMessage;
     *
     * let mut buffer: Vec<u8> = Vec::new();
     * let mut cursor = std::io::Cursor::new(buffer);
     *
     * let message = HybridPirMessage::Seed(1234);
     * message.write_to(&mut cursor).unwrap();
     *
     * cursor.seek(SeekFrom::Start(0)).unwrap();
     *
     * let deserialized = HybridPirMessage::read_from(&mut cursor).unwrap();
     * assert!(deserialized == HybridPirMessage::Seed(1234));
     * ```
     */
    pub fn read_from<R: Read>(mut stream: &mut R) -> Result<Self, std::io::Error> {
        bincode::deserialize_from(&mut stream)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{}", e)))
    }
}

// Everything below this point is just for the purposes of benchmarks

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RaidPirMessage {
    Hello,
    Seed(u128),
    Query(
        #[serde(with = "serde_bytes")]
        Vec<u8>,
    ),
    Response(Vec<u8>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SealPirMessage {
    Query(
        #[serde(with = "serde_bytes")]
        Vec<u8>,
        PirQuery,
    ),
    Response(PirReply),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum BenchmarkParams {
    SealPir {
        db_size: usize,
        element_size: usize,
        poly_degree: u32,
        log: u32,
        d: u32
    },
    RaidPir {
        db_size: usize,
        element_size: usize,
        servers: usize,
        redundancy: usize,
        russians: bool
    },
    HybridPir{
        db_size: usize,
        element_size: usize,
        raidpir_servers: usize,
        raidpir_redundancy: usize,
        raidpir_size: usize,
        raidpir_russians: bool,
        sealpir_poly_degree: u32,
        sealpir_log: u32,
        sealpir_d: u32
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ProtocolMessage {
    SealPir(SealPirMessage),
    RaidPir(RaidPirMessage),
    HybridPir(HybridPirMessage),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum BenchmarkMessage {
    Setup(BenchmarkParams),
    RefreshQueue,
    Ready,
    Protocol(ProtocolMessage),
}

impl BenchmarkMessage {
    pub fn write_to<W: Write>(&self, mut stream: &mut W) -> Result<(), std::io::Error> {
        bincode::serialize_into(&mut stream, self)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{}", e)))
    }

    pub fn read_from<R: Read>(mut stream: &mut R) -> Result<Self, std::io::Error> {
        bincode::deserialize_from(&mut stream)
            .map_err(|e| Error::new(ErrorKind::Other, format!("{}", e)))
    }
}
