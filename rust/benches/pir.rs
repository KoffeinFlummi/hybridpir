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

fn bench_pem_100k(c: &mut Criterion) {
    let mut group = c.benchmark_group("PEM_100k");

    let raidpir_servers = 2;
    let raidpir_redundancy = 2;

    let size_bytes = 1 << 29; // 512 MiB (next power of two from 366.21 MiB)

    let connection: Option<(f32, f32)> = None;
    //let connection = Some((25.0, 10.0));

    for elements_exp in [8,10,12,14,16,18,20,22,24,26].iter() {
        let elements = 1 << elements_exp;
        let element_bytes = size_bytes / elements;
        let index = elements >> 1;

        let mut prng = StdRng::from_entropy();
        let mut db: Vec<Vec<u8>> = Vec::with_capacity(elements);
        for _i in 0..elements {
            let mut buffer = vec![0; element_bytes];
            prng.fill_bytes(&mut buffer);
            db.push(buffer);
        }

        if element_bytes < 3072 {
            group.bench_with_input(BenchmarkId::new(format!("SealPir,n=2^{}", elements_exp), 0), &0, |bench, _| {
                let mut server = PirServer::new(db.len() as u32, element_bytes as u32, 2048, 12, 2);
                let client = PirClient::new(db.len() as u32, element_bytes as u32, 2048, 12, 2);

                {
                    let key = client.get_key();
                    server.set_galois_key(key, 0);
                }

                server.setup(db.clone());

                bench.iter(|| {
                    let query = client.gen_query(index as u32);

                    if let Some((_, speed)) = connection {
                        std::thread::sleep(Duration::from_secs_f32(query.query.len() as f32 * 8.0 / (speed * 1_000_000.0)));
                    }

                    let reply = server.gen_reply(&query, 0);

                    if let Some((speed, _)) = connection {
                        std::thread::sleep(Duration::from_secs_f32(reply.reply.len() as f32 * 8.0 / (speed * 1_000_000.0)));
                    }

                    client.decode_reply(index as u32, &reply);
                });
            });
        }

        group.bench_with_input(BenchmarkId::new(format!("RaidPir,n=2^{}", elements_exp), 0), &0, |bench, _| {
            let raidpir_db: Vec<RaidPirData> = db.iter().map(|x| RaidPirData::new(x.clone())).collect();

            let mut servers: Vec<Arc<RaidPirServer<RaidPirData>>> = (0..raidpir_servers)
                .map(|i| Arc::new(RaidPirServer::new(raidpir_db.clone(), i, raidpir_servers, raidpir_redundancy, false)))
                .collect();

            let client = RaidPirClient::new(db.len(), raidpir_servers, raidpir_redundancy);

            let servers_setup = servers.clone();
            bench.iter_batched(|| {servers_setup.iter().for_each(|s| s.preprocess())}, |()| {
                let seeds = servers.iter_mut().map(|s| s.seed()).collect();

                let queries = client.query(index, &seeds);

                if let Some((_, speed)) = connection {
                    let len = &queries[0].clone().into_vec().len();
                    std::thread::sleep(Duration::from_secs_f32(*len as f32 * 8.0 / (speed * 1_000_000.0)));
                }

                let responses: Vec<RaidPirData> = servers
                    .par_iter_mut()
                    .zip(seeds.par_iter().zip(queries.par_iter()))
                    .map(|(server, (seed, query))| server.response(*seed, query))
                    .with_max_len(1)
                    .collect();

                if let Some((speed, _)) = connection {
                    std::thread::sleep(Duration::from_secs_f32(db[0].len() as f32 * 8.0 / (speed * 1_000_000.0)));
                }

                client.combine(responses);
            }, BatchSize::NumIterations(32));
        });

        for sealpir_size_exp in [2, 4, 6, 8, 10, 12, 14].iter() {
            let exp = elements_exp - sealpir_size_exp;
            let raidpir_size = 1usize << exp;

            if raidpir_size % (raidpir_servers * 8) != 0 || raidpir_size >= db.len() {
                continue;
            }

            if element_bytes >= 3072 {
                continue;
            }

            group.bench_with_input(BenchmarkId::new(format!("HybridPir,n=2^{}", elements_exp), sealpir_size_exp), &raidpir_size, |bench, raidpir_size| {
                let mut servers: Vec<Arc<HybridPirServer>> = (0..raidpir_servers)
                    .map(|i| Arc::new(HybridPirServer::new(
                        &db,
                        i, raidpir_servers, raidpir_redundancy, *raidpir_size, false,
                        2048, 12, 2
                    ))).collect();

                let client = HybridPirClient::new(db.len(), element_bytes,
                    raidpir_servers, raidpir_redundancy, *raidpir_size,
                    2048, 12, 2);

                let servers_setup = servers.clone();
                bench.iter_batched(|| {servers_setup.iter().for_each(|s| s.preprocess())}, |()| {
                    let seeds = servers.iter_mut().map(|s| s.seed()).collect();

                    let (raidpir_queries, sealpir_query) = client.query(index, &seeds);

                    if let Some((_, speed)) = connection {
                        let len = &raidpir_queries[0].clone().into_vec().len();
                        std::thread::sleep(Duration::from_secs_f32(*len as f32 * 8.0 / (speed * 1_000_000.0)));
                        std::thread::sleep(Duration::from_secs_f32(sealpir_query.query.len() as f32 * 8.0 / (speed * 1_000_000.0)));
                    }

                    let sealpir_key = client.sealpir_key();

                    let responses: Vec<PirReply> = servers
                        .par_iter_mut()
                        .zip(seeds.par_iter().zip(raidpir_queries.par_iter()))
                        .map(|(server, (seed, raidpir_query))| server.response(*seed, raidpir_query, sealpir_key, &sealpir_query))
                        .with_max_len(1)
                        .collect();

                    if let Some((speed, _)) = connection {
                        std::thread::sleep(Duration::from_secs_f32(responses[0].reply.len() as f32 * 8.0 / (speed * 1_000_000.0)));
                    }

                    client.combine(index, responses);
                }, BatchSize::NumIterations(32));
            });
        }
    }
}

fn bench_online(c: &mut Criterion) {
    let mut group = c.benchmark_group("Online");
    group.plot_config(PlotConfiguration::default()
        .summary_scale(AxisScale::Logarithmic));

    let raidpir_servers = 4;
    let raidpir_redundancy = 2;
    let element_size = 8;

    let connection: Option<(f32, f32)> = None;
    //let connection = Some((25.0, 10.0));

    for size_exp in [18, 20, 22, 24, 26, 28, 30].iter() {
        let size = 1 << size_exp;
        let index = size >> 1;

        let mut prng = StdRng::from_entropy();

        let mut db: Vec<Vec<u8>> = Vec::with_capacity(size);
        for _i in 0..size {
            let mut buffer = vec![0; element_size];
            prng.fill_bytes(&mut buffer);
            db.push(buffer);
        }

        if element_size == 8 {
            db[index] = b"deadbeef".to_vec();
        }

        // TODO: fill queues before benchmark?

        group.bench_with_input(BenchmarkId::new(format!("SealPir,n=2^{}", size_exp), 0), &0, |bench, _| {
            let mut server = PirServer::new(db.len() as u32, 8, 2048, 12, 2);
            let client = PirClient::new(db.len() as u32, 8, 2048, 12, 2);

            {
                let key = client.get_key();
                server.set_galois_key(key, 0);
            }

            let mut collection: Vec<[u8; 4]> = Vec::with_capacity(size);
            for i in 0..size {
                collection.push(db[i].clone().try_into().unwrap());
            }

            server.setup(collection);

            bench.iter(|| {
                let query = client.gen_query(index as u32);

                if let Some((_, speed)) = connection {
                    std::thread::sleep(Duration::from_secs_f32(query.query.len() as f32 * 8.0 / (speed * 1_000_000.0)));
                }

                let reply = server.gen_reply(&query, 0);

                if let Some((speed, _)) = connection {
                    std::thread::sleep(Duration::from_secs_f32(reply.reply.len() as f32 * 8.0 / (speed * 1_000_000.0)));
                }

                // TODO: this sometimes crashes, reproducible outside of benchmarks?
                client.decode_reply(index as u32, &reply);
            });
        });

        group.bench_with_input(BenchmarkId::new(format!("RaidPir,n=2^{}", size_exp), 0), &0, |bench, _| {
            let raidpir_db: Vec<RaidPirData> = db.iter().map(|x| RaidPirData::new(x.clone())).collect();

            let mut servers: Vec<RaidPirServer<RaidPirData>> = (0..raidpir_servers)
                .map(|i| RaidPirServer::new(raidpir_db.clone(), i, raidpir_servers, raidpir_redundancy, true))
                .collect();

            let client = RaidPirClient::new(db.len(), raidpir_servers, raidpir_redundancy);

            bench.iter(|| {
                let seeds = servers.iter_mut().map(|s| s.seed()).collect();

                let queries = client.query(index, &seeds);

                if let Some((_, speed)) = connection {
                    let len = &queries[0].clone().into_vec().len() * 4;
                    std::thread::sleep(Duration::from_secs_f32(len as f32 * 8.0 / (speed * 1_000_000.0)));
                }

                let responses: Vec<RaidPirData> = servers
                    .par_iter_mut()
                    .zip(seeds.par_iter().zip(queries.par_iter()))
                    .map(|(server, (seed, query))| server.response(*seed, query))
                    .with_max_len(1)
                    .collect();

                if let Some((speed, _)) = connection {
                    std::thread::sleep(Duration::from_secs_f32(db[0].len() as f32 * 8.0 / (speed * 1_000_000.0)));
                }

                client.combine(responses);
            });
        });

        for sealpir_size_exp in [4, 6, 8, 10, 12, 14, 16].iter() {
            let exp = size_exp - sealpir_size_exp;
            let raidpir_size = 1usize << exp;

            group.bench_with_input(BenchmarkId::new(format!("HybridPir,n=2^{}", size_exp), sealpir_size_exp), &raidpir_size, |bench, raidpir_size| {
                let mut servers: Vec<HybridPirServer> = (0..raidpir_servers)
                    .map(|i| HybridPirServer::new(
                        &db,
                        i, raidpir_servers, raidpir_redundancy, *raidpir_size, true,
                        2048, 12, 2
                    )).collect();

                let client = HybridPirClient::new(db.len(), 8,
                    raidpir_servers, raidpir_redundancy, *raidpir_size,
                    2048, 12, 2);

                bench.iter(|| {
                    let seeds = servers.iter_mut().map(|s| s.seed()).collect();

                    let (raidpir_queries, sealpir_query) = client.query(index, &seeds);

                    if let Some((_, speed)) = connection {
                        let len = &raidpir_queries[0].clone().into_vec().len() * 4;
                        std::thread::sleep(Duration::from_secs_f32(len as f32 * 8.0 / (speed * 1_000_000.0)));
                        std::thread::sleep(Duration::from_secs_f32(sealpir_query.query.len() as f32 * 8.0 / (speed * 1_000_000.0)));
                    }

                    let sealpir_key = client.sealpir_key();

                    let responses: Vec<PirReply> = servers
                        .par_iter_mut()
                        .zip(seeds.par_iter().zip(raidpir_queries.par_iter()))
                        .map(|(server, (seed, raidpir_query))| server.response(*seed, raidpir_query, sealpir_key, &sealpir_query))
                        .with_max_len(1)
                        .collect();

                    if let Some((speed, _)) = connection {
                        std::thread::sleep(Duration::from_secs_f32(responses[0].reply.len() as f32 * 8.0 / (speed * 1_000_000.0)));
                    }

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
                    i, raidpir_servers, raidpir_redundancy, raidpir_size, true,
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
                        raidpir_queries[0].clone().into_vec(),
                        sealpir_key.clone(),
                        sealpir_query.clone());
                    let _serialized = bincode::serialize(&msg).unwrap();
                });
            });

            let msg = HybridPirMessage::Query(
                raidpir_queries[0].clone().into_vec(),
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

criterion_group!(benches, bench_query, bench_pem_100k, bench_online, bench_serialization);
criterion_main!(benches);
