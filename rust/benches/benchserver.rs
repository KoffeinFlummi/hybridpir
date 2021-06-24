use std::convert::TryInto;
use std::time::Duration;
use std::sync::Arc;

use criterion::*;

use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use rayon::iter::ParallelIterator;
use rayon::iter::IndexedParallelIterator;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::IntoParallelRefMutIterator;

use sealpir::client::PirClient;
use sealpir::server::PirServer;
use sealpir::PirReply;

use raidpir::client::RaidPirClient;
use raidpir::server::RaidPirServer;
use raidpir::types::RaidPirData;

use hybridpir::client::HybridPirClient;
use hybridpir::server::HybridPirServer;
use hybridpir::types::*;

fn bench_ser_query_raidpir(c: &mut Criterion) {
    let mut group = c.benchmark_group("serde_RaidPir");

    let query: Vec<u8> = [0; 65536].to_vec();
    let response: Vec<u8> = [0; 1024].to_vec();

    group.bench_function("ser_query", |bench| {
        bench.iter(|| {
            let msg = BenchmarkMessage::Protocol(ProtocolMessage::RaidPir(RaidPirMessage::Query(query.clone())));
            let _serialized = bincode::serialize(&msg).unwrap();
        });
    });

    group.bench_function("deser_query", |bench| {
        let msg = BenchmarkMessage::Protocol(ProtocolMessage::RaidPir(RaidPirMessage::Query(query.clone())));
        let serialized = bincode::serialize(&msg).unwrap();

        bench.iter(|| {
            let _msg: BenchmarkMessage = bincode::deserialize(&serialized).unwrap();
        });
    });

    group.bench_function("ser_response", |bench| {
        bench.iter(|| {
            let msg = BenchmarkMessage::Protocol(ProtocolMessage::RaidPir(RaidPirMessage::Response(response.clone())));
            let _serialized = bincode::serialize(&msg).unwrap();
        });
    });

    group.bench_function("deser_response", |bench| {
        let msg = BenchmarkMessage::Protocol(ProtocolMessage::RaidPir(RaidPirMessage::Response(response.clone())));
        let serialized = bincode::serialize(&msg).unwrap();

        bench.iter(|| {
            let _msg: BenchmarkMessage = bincode::deserialize(&serialized).unwrap();
        });
    });
}

criterion_group!(benches, bench_ser_query_raidpir);
criterion_main!(benches);
