use hybridpir::client::HybridPirClient;

fn main() {
    env_logger::init();

    let index = std::env::args().nth(1).unwrap().parse().unwrap();

    let size = 1 << 22;
    let raidpir_servers = 2;
    let raidpir_redundancy = 2;
    let raidpir_size = 1 << 12;

    let client = HybridPirClient::new(size, 8,
        raidpir_servers, raidpir_redundancy, raidpir_size,
        2048, 12, 2);

    for _i in 0..1 {
        let response = client
            .send_query(&[("127.0.0.1", 7000), ("127.0.0.1", 7001)], index)
            .unwrap();

        println!("{:02x?}, {:?}", response, String::from_utf8_lossy(&response));
    }
}
