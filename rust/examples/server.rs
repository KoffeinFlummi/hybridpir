use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

use hybridpir::server::HybridPirServer;

fn main() {
    env_logger::init();

    let id = std::env::args().nth(1).unwrap().parse().unwrap();

    let mut prng = StdRng::seed_from_u64(1234);

    let size = 1 << 22;
    let raidpir_servers = 2;
    let raidpir_redundancy = 2;
    let raidpir_size = 1 << 12;

    let mut db: Vec<Vec<u8>> = Vec::with_capacity(size);
    for _i in 0..size {
        let mut buffer = vec![0; 8];
        prng.fill_bytes(&mut buffer);
        db.push(buffer);
    }
    db[size >> 1] = b"deadbeef".to_vec();

    let server = HybridPirServer::new(&db,
        id, raidpir_servers, raidpir_redundancy, raidpir_size, false,
        2048, 12, 2);

    server.accept_connections(("0.0.0.0", (7000 + id) as u16)).unwrap();
}
