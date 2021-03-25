use std::io::{Error, ErrorKind};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bitvec::prelude::*;
use raidpir::server::RaidPirServer;
use raidpir::types::RaidPirData;
use sealpir::server::PirServer;
use sealpir::{PirQuery, PirReply};

use crate::types::*;

#[derive(Debug, Clone)]
pub struct HybridPirServer {
    db_len: usize,
    element_size: usize,
    raidpir: Arc<RaidPirServer<RaidPirData>>,
    raidpir_size: usize,
    sealpir_poly_degree: u32,
    sealpir_log: u32,
    sealpir_d: u32,
}

impl HybridPirServer {
    pub fn new(
        db: &Vec<Vec<u8>>,
        raidpir_id: usize,
        raidpir_servers: usize,
        raidpir_redundancy: usize,
        raidpir_size: usize,
        raidpir_russians: bool,
        sealpir_poly_degree: u32,
        sealpir_log: u32,
        sealpir_d: u32,
    ) -> Self {
        assert!(db.len() > 0);
        assert!(raidpir_size < db.len());
        assert!(raidpir_size % (raidpir_servers * 8) == 0);

        let raidpir_chunksize = (db.len() as f32 / raidpir_size as f32).ceil() as usize;

        let raidpir_db: Vec<RaidPirData> = db
            .chunks(raidpir_chunksize)
            .map(|x| RaidPirData::new(x.into_iter().cloned().flatten().collect::<Vec<u8>>()))
            .collect();

        let raidpir = RaidPirServer::new(
            raidpir_db,
            raidpir_id,
            raidpir_servers,
            raidpir_redundancy,
            raidpir_russians);

        Self {
            db_len: db.len(),
            element_size: db[0].len(),
            raidpir: Arc::new(raidpir),
            raidpir_size,
            sealpir_poly_degree,
            sealpir_log,
            sealpir_d,
        }
    }

    pub fn seed(&self) -> u64 {
        self.raidpir.seed()
    }

    pub fn response(&self,
        seed: u64,
        raidpir_query: &BitVec<Lsb0, u8>,
        sealpir_key: &Vec<u8>,
        sealpir_query: &PirQuery
    ) -> PirReply {
        let mut response: Vec<u8> = self.raidpir
            .response(seed, &raidpir_query)
            .into();

        let raidpir_chunksize = (self.db_len as f32 / self.raidpir_size as f32).ceil() as usize;

        // resize response so every element is full-size
        response.resize(raidpir_chunksize * self.element_size, 0);

        let mut sealpir = PirServer::new(
            raidpir_chunksize as u32,
            self.element_size as u32,
            self.sealpir_poly_degree,
            self.sealpir_log,
            self.sealpir_d
        );

        sealpir.set_galois_key(sealpir_key, 0);
        sealpir.setup(response);

        sealpir.gen_reply(sealpir_query, 0)
    }

    pub fn accept_connections<A: ToSocketAddrs>(self, addr: A) -> Result<(), Error> {
        let listener = TcpListener::bind(addr)?;

        debug!("Listening on {:?}...", listener.local_addr().unwrap());

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let x = self.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = x.handle_connection(stream) {
                            error!("{:?}", e);
                        }
                    });
                },
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        }

        Ok(())
    }

    pub fn handle_connection(&self, mut stream: TcpStream) -> Result<(), Error> {
        let t0 = Instant::now();

        stream.set_read_timeout(Some(Duration::from_secs(60)))?;
        stream.set_write_timeout(Some(Duration::from_secs(60)))?;

        debug!("[{:?}] Accepting connection, waiting for hello...", stream.peer_addr().unwrap());

        // Receive init message.
        match HybridPirMessage::read_from(&mut stream)? {
            HybridPirMessage::Hello => {},
            _ => {
                return Err(Error::new(ErrorKind::Other, "Unexpected Response."));
            }
        }

        debug!("[{:?}] Received hello ({:.4}ms), sending seed...",
            stream.peer_addr().unwrap(),
            t0.elapsed().as_secs_f64() * 1000.0);

        let t1 = Instant::now();

        // Send seeds
        let seed = self.seed();
        let msg = HybridPirMessage::Seed(seed);
        msg.write_to(&mut stream)?;

        debug!("[{:?}] Seed sent ({:.4}ms), waiting for query...",
            stream.peer_addr().unwrap(),
            t1.elapsed().as_secs_f64() * 1000.0);

        let t2 = Instant::now();

        // Receive query
        let (raidpir_query, sealpir_key, sealpir_query) = match HybridPirMessage::read_from(&mut stream)? {
            HybridPirMessage::Query(a,b,c) => Ok((a,b,c)),
            _ => Err(Error::new(ErrorKind::Other, "Unexpected Response."))
        }?;

        // Convert raidpir_query to bitvec
        assert!(std::mem::size_of::<&usize>() == std::mem::size_of::<&u64>());
        let raidpir_query: BitVec<Lsb0, u8> = BitVec::from_vec(raidpir_query);

        debug!("[{:?}] Received query ({:.4}ms), calculating response...",
            stream.peer_addr().unwrap(),
            t2.elapsed().as_secs_f64() * 1000.0);

        let t3 = Instant::now();

        let response = self.response(seed, &raidpir_query, &sealpir_key, &sealpir_query);

        debug!("[{:?}] Calculated response ({:.4}ms), sending response...",
            stream.peer_addr().unwrap(),
            t3.elapsed().as_secs_f64() * 1000.0);

        let t4 = Instant::now();

        let msg = HybridPirMessage::Response(response);
        msg.write_to(&mut stream)?;

        stream.shutdown(std::net::Shutdown::Both)?;

        debug!("[{:?}] Sent response ({:.4}ms). Total elapsed: {:.4}ms",
            stream.peer_addr().unwrap(),
            t4.elapsed().as_secs_f64() * 1000.0,
            t0.elapsed().as_secs_f64() * 1000.0);

        // Done, use this thread to rebuild RaidPir queue
        self.raidpir.preprocess();

        Ok(())
    }
}
