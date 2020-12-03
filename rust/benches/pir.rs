use std::convert::TryInto;

use criterion::*;

use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use sealpir::client::PirClient;
use sealpir::server::PirServer;
use sealpir::PirReply;

use hybridpir::client::HybridPirClient;
use hybridpir::server::HybridPirServer;
use hybridpir::types::*;

fn bench_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("Query");
    group.plot_config(PlotConfiguration::default()
        .summary_scale(AxisScale::Logarithmic));

    let size = 1 << 30;
    let raidpir_servers = 2;
    let raidpir_redundancy = 2;
    let index = size >> 1;

    for exp in [8, 12, 16, 20, 24, 28].iter() {
        let raidpir_size = 1usize << exp;

        group.bench_with_input(BenchmarkId::new("HybridPir", raidpir_size), &raidpir_size, |bench, raidpir_size| {
            let client = HybridPirClient::new(size, 8,
                raidpir_servers, raidpir_redundancy, *raidpir_size,
                2048, 12, 2);

            let seeds = vec![1234, 4321];

            bench.iter(|| client.query(index, &seeds));
        });
    }

    group.bench_with_input(BenchmarkId::new("SealPir", 1 << 8), &0, |bench, _| {
        let client = PirClient::new(size as u32, 8, 2048, 12, 2);

        bench.iter(|| client.gen_query(index as u32));
    });

    group.bench_with_input(BenchmarkId::new("SealPir", 1 << 28), &0, |bench, _| {
        let client = PirClient::new(size as u32, 8, 2048, 12, 2);

        bench.iter(|| client.gen_query(index as u32));
    });
}

fn bench_online(c: &mut Criterion) {
    let mut group = c.benchmark_group("Online");
    group.plot_config(PlotConfiguration::default()
        .summary_scale(AxisScale::Logarithmic));

    for size in [1 << 20, 1 << 22].iter() {
        let raidpir_servers = 2;
        let raidpir_redundancy = 2;
        let index = size >> 1;

        let mut prng = StdRng::from_entropy();

        let mut db: Vec<Vec<u8>> = Vec::with_capacity(*size);
        for _i in 0..*size {
            let mut buffer = vec![0; 8];
            prng.fill_bytes(&mut buffer);
            db.push(buffer);
        }
        db[index] = b"deadbeef".to_vec();

        group.bench_with_input(BenchmarkId::new(format!("SealPir,n={}", size), 1 << 12), &0, |bench, _| {
            let mut server = PirServer::new(db.len() as u32, 8, 2048, 12, 2);
            let client = PirClient::new(db.len() as u32, 8, 2048, 12, 2);

            {
                let key = client.get_key();
                server.set_galois_key(key, 0);
            }

            let mut collection: Vec<[u8; 8]> = Vec::with_capacity(*size);
            for i in 0..*size {
                collection.push(db[i].clone().try_into().unwrap());
            }

            server.setup(collection);

            bench.iter(|| {
                let query = client.gen_query(index as u32);
                let reply = server.gen_reply(&query, 0);

                // TODO: this sometimes crashes, reproducible outside of benchmarks?
                client.decode_reply(index as u32, &reply);
            });
        });

        for exp in [8, 9, 10, 11, 12, 13, 14, 15, 16].iter() {
            let raidpir_size = 1usize << exp;

            group.bench_with_input(BenchmarkId::new(format!("HybridPir,n={}", size), raidpir_size), &raidpir_size, |bench, raidpir_size| {
                let mut servers: Vec<HybridPirServer> = (0..raidpir_servers)
                    .map(|i| HybridPirServer::new(
                        &db,
                        i, raidpir_servers, raidpir_redundancy, *raidpir_size,
                        2048, 12, 2
                    )).collect();

                let client = HybridPirClient::new(db.len(), 8,
                    raidpir_servers, raidpir_redundancy, *raidpir_size,
                    2048, 12, 2);

                bench.iter(|| {
                    let seeds = servers.iter_mut().map(|s| s.seed()).collect();

                    let (raidpir_queries, sealpir_query) = client.query(index, &seeds);

                    let sealpir_key = client.sealpir_key();

                    let responses: Vec<PirReply> = servers
                        .iter_mut()
                        .zip(seeds.iter().zip(raidpir_queries.iter()))
                        .map(|(server, (seed, raidpir_query))| server.response(*seed, raidpir_query, sealpir_key, &sealpir_query))
                        .collect();

                    // TODO: see above
                    client.combine(index, responses);
                });
            });
        }
    }
}

fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("Serialization");
    group.plot_config(PlotConfiguration::default()
        .summary_scale(AxisScale::Logarithmic));

    for size in [1 << 20, 1 << 22].iter() {
        let raidpir_servers = 2;
        let raidpir_redundancy = 2;
        let index = size >> 1;

        let mut prng = StdRng::from_entropy();

        let mut db: Vec<Vec<u8>> = Vec::with_capacity(*size);
        for _i in 0..*size {
            let mut buffer = vec![0; 8];
            prng.fill_bytes(&mut buffer);
            db.push(buffer);
        }
        db[index] = b"deadbeef".to_vec();

        for exp in [8, 10, 12, 14, 16].iter() {
            let raidpir_size = 1usize << exp;

            let mut servers: Vec<HybridPirServer> = (0..raidpir_servers)
                .map(|i| HybridPirServer::new(
                    &db,
                    i, raidpir_servers, raidpir_redundancy, raidpir_size,
                    2048, 12, 2
                )).collect();

            let client = HybridPirClient::new(db.len(), 8,
                raidpir_servers, raidpir_redundancy, raidpir_size,
                2048, 12, 2);

            let seeds = servers.iter_mut().map(|s| s.seed()).collect();

            let (raidpir_queries, sealpir_query) = client.query(index, &seeds);

            let sealpir_key = client.sealpir_key();

            group.bench_with_input(BenchmarkId::new(format!("serialize_query,n={}", size), raidpir_size), &0, |bench, _| {
                bench.iter(|| {
                    let msg = HybridPirMessage::Query(
                        bitvec_to_u64(&raidpir_queries[0]),
                        sealpir_key.clone(),
                        sealpir_query.clone());
                    let _serialized = bincode::serialize(&msg).unwrap();
                });
            });

            let msg = HybridPirMessage::Query(
                bitvec_to_u64(&raidpir_queries[0]),
                sealpir_key.clone(),
                sealpir_query.clone());
            let serialized = bincode::serialize(&msg).unwrap();

            group.bench_with_input(BenchmarkId::new(format!("deserialize_query,n={}", size), raidpir_size), &0, |bench, _| {
                bench.iter(|| {
                    let _msg: HybridPirMessage = bincode::deserialize(&serialized).unwrap();
                });
            });

            let responses: Vec<PirReply> = servers
                .iter_mut()
                .zip(seeds.iter().zip(raidpir_queries.iter()))
                .map(|(server, (seed, raidpir_query))| server.response(*seed, raidpir_query, sealpir_key, &sealpir_query))
                .collect();

            group.bench_with_input(BenchmarkId::new(format!("serialize_response,n={}", size), raidpir_size), &0, |bench, _| {
                bench.iter(|| {
                    let msg = HybridPirMessage::Response(responses[0].clone());
                    let _serialized = bincode::serialize(&msg).unwrap();
                });
            });

            let msg = HybridPirMessage::Response(responses[0].clone());
            let serialized = bincode::serialize(&msg).unwrap();

            group.bench_with_input(BenchmarkId::new(format!("deserialize_response,n={}", size), raidpir_size), &0, |bench, _| {
                bench.iter(|| {
                    let _msg: HybridPirMessage = bincode::deserialize(&serialized).unwrap();
                });
            });

            client.combine(index, responses);
        }
    }
}

criterion_group!(benches, bench_query, bench_online, bench_serialization);
criterion_main!(benches);
