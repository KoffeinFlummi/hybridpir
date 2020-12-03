use std::net::SocketAddr;

use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use sealpir::PirReply;

use hybridpir::client::HybridPirClient;
use hybridpir::server::HybridPirServer;

#[test]
fn test_pir() {
    let mut prng = StdRng::from_entropy();

    let size = 1 << 20;
    let raidpir_servers = 2;
    let raidpir_redundancy = 2;
    let raidpir_size = 1 << 8;
    let index = size >> 1;

    let mut db: Vec<Vec<u8>> = Vec::with_capacity(size);
    for _i in 0..size {
        let mut buffer = vec![0; 8];
        prng.fill_bytes(&mut buffer);
        db.push(buffer);
    }
    db[index] = b"deadbeef".to_vec();

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

    let mut responses: Vec<PirReply> = servers
        .iter_mut()
        .zip(seeds.iter().zip(raidpir_queries.iter()))
        .map(|(server, (seed, raidpir_query))| server.response(*seed, raidpir_query, sealpir_key, &sealpir_query))
        .collect();

    let response = client.combine(index, responses);
    assert!(response == b"deadbeef");
}

#[test]
fn test_tcp() {
    let mut prng = StdRng::from_entropy();

    let size = 1 << 20;
    let raidpir_servers = 2;
    let raidpir_redundancy = 2;
    let raidpir_size = 1 << 8;
    let index = size >> 1;

    let mut db: Vec<Vec<u8>> = Vec::with_capacity(size);
    for _i in 0..size {
        let mut buffer = vec![0; 8];
        prng.fill_bytes(&mut buffer);
        db.push(buffer);
    }
    db[index] = b"deadbeef".to_vec();

    for i in (0..raidpir_servers) {
        let server = HybridPirServer::new(&db,
            i, raidpir_servers, raidpir_redundancy, raidpir_size,
            2048, 12, 2);

        std::thread::spawn(move || {
            server.accept_connections(("localhost", (7000 + i) as u16)).unwrap();
        });
    }

    let client = HybridPirClient::new(db.len(), 8,
        raidpir_servers, raidpir_redundancy, raidpir_size,
        2048, 12, 2);

    let response = client
        .send_query(&[("localhost", 7000), ("localhost", 7001)], index)
        .unwrap();

    assert!(response == b"deadbeef");
}
