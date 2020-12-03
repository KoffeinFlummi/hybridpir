use std::io::{Error, ErrorKind, Read, Write};

use bitvec::prelude::*;
use sealpir::{PirQuery, PirReply};
use serde::{Serialize, Deserialize};
use bincode;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum HybridPirMessage {
    Hello,
    Seed(u64),
    Query(
        Vec<u64>,
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

pub fn bitvec_to_u64(bitvec: &BitVec<Lsb0, usize>) -> Vec<u64> {
    if cfg!(target_pointer_width = "64") {
        bitvec.as_raw_slice()
            .iter()
            .map(|x| *x as u64)
            .collect()
    } else {
        let slice = bitvec.as_raw_slice();
        assert!(slice.len() % 2 == 0);

        slice.chunks(2)
            .map(|chunk| (chunk[1] as u64) << 32 + (chunk[0] as u64))
            .collect()
    }
}
