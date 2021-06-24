use std::io::Error;
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use bitvec::prelude::*;

use sealpir::server::PirServer;
use raidpir::server::RaidPirServer;
use raidpir::types::RaidPirData;
use hybridpir::server::HybridPirServer;
use hybridpir::types::*;

use log::*;

enum BenchmarkServer<'a> {
    SealPir(PirServer<'a>),
    RaidPir(RaidPirServer<RaidPirData>, u64),
    HybridPir(HybridPirServer, u64),
}

impl BenchmarkServer<'_> {
    fn setup_db(db_size: usize, element_size: usize) -> Vec<Vec<u8>> {
        let mut prng = StdRng::seed_from_u64(1234);

        let mut db: Vec<Vec<u8>> = Vec::with_capacity(db_size);
        for _i in 0..db_size {
            let mut buffer = vec![0; element_size];
            prng.fill_bytes(&mut buffer);
            db.push(buffer);
        }

        db
    }

    pub fn setup(id: usize, params: BenchmarkParams) -> Self {
        match params {
            BenchmarkParams::SealPir{
                db_size,
                element_size,
                poly_degree,
                log,
                d
            } => {
                let mut server = PirServer::new(
                    db_size as u32,
                    element_size as u32,
                    poly_degree,
                    log,
                    d
                );
                let db = Self::setup_db(db_size, element_size);
                server.setup(db);
                BenchmarkServer::SealPir(server)
            },
            BenchmarkParams::RaidPir{
                db_size,
                element_size,
                servers,
                redundancy,
                russians
            } => {
                let db: Vec<RaidPirData> = Self::setup_db(db_size, element_size)
                    .into_iter()
                    .map(|x| RaidPirData::new(x))
                    .collect();

                BenchmarkServer::RaidPir(RaidPirServer::new(
                    db,
                    id,
                    servers,
                    redundancy,
                    russians
                ), 0)
            },
            BenchmarkParams::HybridPir{
                db_size,
                element_size,
                raidpir_servers,
                raidpir_redundancy,
                raidpir_size,
                raidpir_russians,
                sealpir_poly_degree,
                sealpir_log,
                sealpir_d
            } => {
                let db = Self::setup_db(db_size, element_size);

                BenchmarkServer::HybridPir(HybridPirServer::new(
                    &db,
                    id,
                    raidpir_servers,
                    raidpir_redundancy,
                    raidpir_size,
                    raidpir_russians,
                    sealpir_poly_degree,
                    sealpir_log,
                    sealpir_d
                ), 0)
            }
        }
    }

    pub fn refresh_queue(&mut self) {
        match self {
            Self::SealPir(ref mut _server) => {}
            Self::RaidPir(ref mut server, _seed) => {
                server.preprocess();
            },
            Self::HybridPir(ref mut server, _seed) => {
                server.preprocess();
            },
        }
    }

    pub fn handle_msg(&mut self, msg: ProtocolMessage) -> Option<ProtocolMessage> {
        match msg {
            ProtocolMessage::SealPir(sealpir_msg) => {
                if let BenchmarkServer::SealPir(ref mut server) = self {
                    if let SealPirMessage::Query(key, query) = sealpir_msg {
                        let t = std::time::Instant::now();
                        server.set_galois_key(&key, 0);
                        let reply = server.gen_reply(&query, 0);
                        debug!("Response time: {:?}", t.elapsed().as_secs_f64() * 1000.0);
                        Some(ProtocolMessage::SealPir(SealPirMessage::Response(reply)))
                    } else {
                        unreachable!();
                    }
                } else {
                    unreachable!();
                }
            },
            ProtocolMessage::RaidPir(raidpir_msg) => {
                if let BenchmarkServer::RaidPir(ref mut server, ref mut seed) = self {
                    if let RaidPirMessage::Hello = raidpir_msg {
                        let t = std::time::Instant::now();
                        *seed = server.seed();
                        debug!("Seed time: {:?}", t.elapsed().as_secs_f64() * 1000.0);
                        Some(ProtocolMessage::RaidPir(RaidPirMessage::Seed(*seed)))
                    } else if let RaidPirMessage::Query(query) = raidpir_msg {
                        let t = std::time::Instant::now();
                        let bitvec: BitVec<Lsb0, u8> = BitVec::from_vec(query);
                        let response: Vec<u8> = server.response(*seed, &bitvec).into();
                        debug!("Response time: {:?}", t.elapsed().as_secs_f64() * 1000.0);
                        Some(ProtocolMessage::RaidPir(RaidPirMessage::Response(response)))
                    } else {
                        unreachable!();
                    }
                } else {
                    unreachable!();
                }
            },
            ProtocolMessage::HybridPir(hybridpir_msg) => {
                if let BenchmarkServer::HybridPir(ref mut server, ref mut seed) = self {
                    if let HybridPirMessage::Hello = hybridpir_msg {
                        let t = std::time::Instant::now();
                        *seed = server.seed();
                        debug!("Seed time: {:?}", t.elapsed().as_secs_f64() * 1000.0);
                        Some(ProtocolMessage::HybridPir(HybridPirMessage::Seed(*seed)))
                    } else if let HybridPirMessage::Query(raidpir_query, sealpir_key, sealpir_query) = hybridpir_msg {
                        let t = std::time::Instant::now();
                        let bitvec: BitVec<Lsb0, u8> = BitVec::from_vec(raidpir_query);
                        let response = server.response(*seed, &bitvec, &sealpir_key, &sealpir_query);
                        debug!("Response time: {:?}", t.elapsed().as_secs_f64() * 1000.0);
                        Some(ProtocolMessage::HybridPir(HybridPirMessage::Response(response)))
                    } else {
                        unreachable!();
                    }
                } else {
                    unreachable!();
                }
            },
        }
    }
}

pub fn handle_connection(id: usize, mut stream: TcpStream) -> Result<(), Error> {
    stream.set_read_timeout(Some(Duration::from_secs(3600)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3600)))?;
    stream.set_nodelay(true)?;

    debug!("[{:?}] Accepting connection", stream.peer_addr().unwrap());

    let mut pir_server: Option<BenchmarkServer> = None;

    loop {
        match BenchmarkMessage::read_from(&mut stream)? {
            BenchmarkMessage::Setup(params) => {
                debug!("Setting up with {:?}", params);
                pir_server = Some(BenchmarkServer::setup(id, params));
                let response = BenchmarkMessage::Ready;
                response.write_to(&mut stream)?;
            },
            BenchmarkMessage::RefreshQueue => {
                debug!("Refreshing queue...");
                if let Some(ref mut server) = pir_server {
                    server.refresh_queue();
                }
                let response = BenchmarkMessage::Ready;
                response.write_to(&mut stream)?;
            },
            BenchmarkMessage::Protocol(msg) => {
                if let Some(ref mut server) = pir_server {
                    let response = server.handle_msg(msg);

                    if let Some(msg) = response {
                        let bm = BenchmarkMessage::Protocol(msg);
                        bm.write_to(&mut stream)?;
                    }
                }
            },
            _ => {
                unreachable!();
            }
        }
    }
}

fn main() {
    env_logger::init();

    let id: usize = std::env::args().nth(1).unwrap().parse().unwrap();

    let listener = TcpListener::bind(("0.0.0.0", 7000 + id as u16)).unwrap();

    debug!("Listening on {:?}...", listener.local_addr().unwrap());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(id, stream) {
                        error!("{:?}", e);
                    }
                });
            },
            Err(e) => {
                error!("{:?}", e);
            }
        }
    }
}
