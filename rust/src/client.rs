use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::io::{Error, ErrorKind};
use std::time::{Duration, Instant};

use bitvec::prelude::*;
use raidpir::client::RaidPirClient;
use raidpir::types::RaidPirData;
use sealpir::client::PirClient;
use sealpir::{PirQuery, PirReply};
use rayon::prelude::*;

use crate::types::*;

pub struct HybridPirClient<'a> {
    db_len: usize,
    raidpir: RaidPirClient,
    raidpir_servers: usize,
    raidpir_chunksize: usize,
    sealpir: PirClient<'a>,
}

impl HybridPirClient<'_> {
    pub fn new(
        db_len: usize,
        element_size: usize,
        raidpir_servers: usize,
        raidpir_redundancy: usize,
        raidpir_size: usize,
        sealpir_poly_degree: u32,
        sealpir_log: u32,
        sealpir_d: u32,
    ) -> Self {
        assert!(raidpir_size < db_len);
        assert!(raidpir_size % (raidpir_servers * 8) == 0);

        let raidpir = RaidPirClient::new(
            raidpir_size,
            raidpir_servers,
            raidpir_redundancy);

        let raidpir_chunksize = (db_len as f32 / raidpir_size as f32).ceil() as usize;

        let sealpir = PirClient::new(
            raidpir_chunksize as u32,
            element_size as u32,
            sealpir_poly_degree as u32,
            sealpir_log as u32,
            sealpir_d as u32);

        Self {
            db_len,
            raidpir,
            raidpir_servers,
            raidpir_chunksize,
            sealpir,
        }
    }

    pub fn sealpir_key(&self) -> &Vec<u8> {
        self.sealpir.get_key()
    }

    pub fn query(&self, index: usize, seeds: &Vec<u64>) -> (Vec<BitVec<Lsb0, u8>>, PirQuery) {
        assert!(index < self.db_len);
        assert!(seeds.len() == self.raidpir_servers);

        let raidpir_index = index / self.raidpir_chunksize;
        let raidpir_queries = self.raidpir.query(raidpir_index, seeds);

        let sealpir_index = index - raidpir_index * self.raidpir_chunksize;
        let sealpir_query = self.sealpir.gen_query(sealpir_index as u32);

        (raidpir_queries, sealpir_query)
    }

    pub fn combine(&self, index: usize, responses: Vec<PirReply>) -> Vec<u8> {
        let raidpir_index = index / self.raidpir_chunksize;
        let sealpir_index = index - raidpir_index * self.raidpir_chunksize;

        let sealpir_responses: Vec<Vec<u8>> = responses.par_iter()
            .with_max_len(1)
            .map(|response| self.sealpir.decode_reply(sealpir_index as u32, &response))
            .collect();

        let raidpir_response = self.raidpir
            .combine(sealpir_responses.into_iter().map(|r| RaidPirData::new(r)).collect());

        raidpir_response.into()
    }

    pub fn send_query<A: ToSocketAddrs>(&self, targets: &[A], index: usize) -> Result<Vec<u8>, Error> {
        let addresses: Vec<SocketAddr> = targets
            .iter()
            .map(|x| x.to_socket_addrs().unwrap().next().unwrap())
            .collect();
        assert!(addresses.len() == self.raidpir_servers);

        // Init connections
        let mut streams: Vec<TcpStream> = addresses
            .par_iter() // Establish connections in parallel
            .map(|target| {
                let stream = TcpStream::connect(target)?;
                stream.set_read_timeout(Some(Duration::from_secs(60)))?;
                stream.set_write_timeout(Some(Duration::from_secs(60)))?;
                stream.set_nodelay(true);
                Ok(stream)
            })
            .with_max_len(1) // Ensure each iteration gets a thread
            .collect::<Result<Vec<TcpStream>, Error>>()?;

        // Send hello message and retrieve seed for each server
        let seeds: Vec<u64> = streams
            .par_iter_mut()
            .map(|mut stream| {
                let hello = HybridPirMessage::Hello;
                hello.write_to(&mut stream)?;

                match HybridPirMessage::read_from(&mut stream)? {
                    HybridPirMessage::Seed(s) => {
                        debug!("[{:?}] Received seed: {:?}.",
                            stream.peer_addr().unwrap(), s);
                        Ok(s)
                    },
                    _ => Err(Error::new(ErrorKind::Other, "Unexpected Response."))
                }
            })
            .with_max_len(1)
            .collect::<Result<Vec<u64>, Error>>()?;

        let t1 = Instant::now();

        debug!("Received all seeds, calculating query...");

        let (raidpir_queries, sealpir_query) = self.query(index, &seeds);

        debug!("Calculated query ({:.4}ms).",
            t1.elapsed().as_secs_f64() * 1000.0);

        // Send queries and retrieve responses
        let responses: Vec<PirReply> = streams
            .par_iter_mut()
            .zip(raidpir_queries.par_iter())
            .map(|(mut stream, raidpir_query)| {
                let t2 = Instant::now();

                debug!("[{:?}] Sending query...", stream.peer_addr().unwrap());

                let message = HybridPirMessage::Query(
                    raidpir_query.clone().into_vec(),
                    self.sealpir_key().clone(), // TODO
                    sealpir_query.clone() // TODO
                );
                message.write_to(&mut stream)?;

                debug!("[{:?}] Sent query ({:.4}ms).",
                    stream.peer_addr().unwrap(),
                    t2.elapsed().as_secs_f64() * 1000.0);

                match HybridPirMessage::read_from(&mut stream)? {
                    HybridPirMessage::Response(r) => Ok(r),
                    _ => Err(Error::new(ErrorKind::Other, "Unexpected Response."))
                }
            })
            .with_max_len(1)
            .collect::<Result<Vec<PirReply>, Error>>()?;

        Ok(self.combine(index, responses))
    }
}
