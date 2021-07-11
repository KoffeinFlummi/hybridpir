use std::io::Error;
use std::net::{SocketAddr, ToSocketAddrs, TcpStream};
use std::time::{Duration, Instant};

use jni::JNIEnv;
use jni::objects::JClass;

use rayon::prelude::*;
use sealpir::client::PirClient;
use sealpir::PirReply;
use raidpir::client::RaidPirClient;
use raidpir::types::RaidPirData;
use crate::client::HybridPirClient;
use crate::types::*;

use log::*;

const QUEUE_SIZE: usize = 32;
//const N: usize = 50;
const N: usize = 10;

const DEFAULT_SEALPIR: BenchmarkParams = BenchmarkParams::SealPir {
    db_size: 1 << 14,
    element_size: 1 << 4,
    poly_degree: 2048,
    log: 12,
    d: 3
};

const DEFAULT_RAIDPIR: BenchmarkParams = BenchmarkParams::RaidPir {
    db_size: 1 << 14,
    element_size: 1 << 4,
    servers: 2,
    redundancy: 2,
    russians: false
};

const DEFAULT_HYBRIDPIR: BenchmarkParams = BenchmarkParams::HybridPir {
    db_size: 1 << 14,
    element_size: 1 << 4,
    raidpir_servers: 2,
    raidpir_redundancy: 2,
    raidpir_size: 1 << 10,
    raidpir_russians: false,
    sealpir_poly_degree: 2048,
    sealpir_log: 24,
    sealpir_d: 1,
};

fn run_query(streams: &mut Vec<TcpStream>, params: &BenchmarkParams) -> Result<(), Error> {
    match params {
        BenchmarkParams::SealPir {
            db_size,
            element_size,
            poly_degree,
            log,
            d
        } => {
            let index = (db_size >> 1) as u32;
            let client = PirClient::new(
                *db_size as u32,
                *element_size as u32,
                *poly_degree,
                *log,
                *d
            );

            let key = client.get_key().clone();
            let t = std::time::Instant::now();
            let query = client.gen_query(index);
            debug!("Query size: {:?}", query.query.len());
            debug!("Query time: {:?}", t.elapsed().as_secs_f64() * 1000.0);
            let msg = BenchmarkMessage::Protocol(ProtocolMessage::SealPir(SealPirMessage::Query(key, query)));

            msg.write_to(&mut streams[0])?;
            let response = BenchmarkMessage::read_from(&mut streams[0])?;

            if let BenchmarkMessage::Protocol(ProtocolMessage::SealPir(SealPirMessage::Response(reply))) = response {
                debug!("Response size: {:?}", reply.reply.len());
                let t = std::time::Instant::now();
                client.decode_reply(index, &reply);
                debug!("Decode time: {:?}", t.elapsed().as_secs_f64() * 1000.0);
            } else {
                unreachable!();
            }
        },
        BenchmarkParams::RaidPir {
            db_size,
            element_size: _,
            servers,
            redundancy,
            russians: _
        } => {
            let index = db_size >> 1;

            let seeds = streams
                .par_iter()
                .map(|ref mut stream| {
                    BenchmarkMessage::Protocol(ProtocolMessage::RaidPir(RaidPirMessage::Hello)).write_to(stream)?;
                    let response = BenchmarkMessage::read_from(stream)?;
                    if let BenchmarkMessage::Protocol(ProtocolMessage::RaidPir(RaidPirMessage::Seed(seed))) = response {
                        Ok(seed)
                    } else {
                        unreachable!();
                    }
                })
                .with_max_len(1)
                .collect::<Result<Vec<u128>, Error>>()?;

            let client = RaidPirClient::new(*db_size, *servers, *redundancy);
            let t = std::time::Instant::now();
            let queries = client.query(index, &seeds);
            debug!("Query size: {:?}", queries[0].len());
            debug!("Query time: {:?}", t.elapsed().as_secs_f64() * 1000.0);

            let responses = streams
                .par_iter()
                .zip(queries.par_iter())
                .map(|(ref mut stream, query)| {
                    BenchmarkMessage::Protocol(ProtocolMessage::RaidPir(RaidPirMessage::Query(query.clone().into_vec()))).write_to(stream)?;
                    let response = BenchmarkMessage::read_from(stream)?;
                    if let BenchmarkMessage::Protocol(ProtocolMessage::RaidPir(RaidPirMessage::Response(resp))) = response {
                        Ok(resp)
                    } else {
                        unreachable!();
                    }
                })
                .with_max_len(1)
                .collect::<Result<Vec<Vec<u8>>, Error>>()?;

            debug!("Response size: {:?}", responses[0].len());

            let t = std::time::Instant::now();
            client.combine(responses.into_iter().map(|r| RaidPirData::new(r)).collect());
            debug!("Decode time: {:?}", t.elapsed().as_secs_f64() * 1000.0);
        },
        BenchmarkParams::HybridPir {
            db_size,
            element_size,
            raidpir_servers,
            raidpir_redundancy,
            raidpir_size,
            raidpir_russians: _,
            sealpir_poly_degree,
            sealpir_log,
            sealpir_d
        } => {
            let index = db_size >> 1;

            let seeds = streams
                .par_iter()
                .map(|ref mut stream| {
                    BenchmarkMessage::Protocol(ProtocolMessage::HybridPir(HybridPirMessage::Hello)).write_to(stream)?;
                    let response = BenchmarkMessage::read_from(stream)?;
                    if let BenchmarkMessage::Protocol(ProtocolMessage::HybridPir(HybridPirMessage::Seed(seed))) = response {
                        Ok(seed)
                    } else {
                        unreachable!();
                    }
                })
                .with_max_len(1)
                .collect::<Result<Vec<u128>, Error>>()?;

            let client = HybridPirClient::new(
                *db_size,
                *element_size,
                *raidpir_servers,
                *raidpir_redundancy,
                *raidpir_size,
                *sealpir_poly_degree,
                *sealpir_log,
                *sealpir_d
            );

            let sealpir_key = client.sealpir_key();
            let t = std::time::Instant::now();
            let (raidpir_queries, sealpir_query) = client.query(index, &seeds);

            debug!("Query size: {:?} (SealPIR)", sealpir_query.query.len());
            debug!("Query size: {:?} (RaidPIR)", raidpir_queries[0].len());
            debug!("Query time: {:?}", t.elapsed().as_secs_f64() * 1000.0);

            let responses = streams
                .par_iter()
                .zip(raidpir_queries.par_iter())
                .map(|(ref mut stream, raidpir_query)| {
                    BenchmarkMessage::Protocol(ProtocolMessage::HybridPir(HybridPirMessage::Query(
                        raidpir_query.clone().into_vec(),
                        sealpir_key.clone(),
                        sealpir_query.clone()
                    ))).write_to(stream)?;
                    let response = BenchmarkMessage::read_from(stream)?;
                    if let BenchmarkMessage::Protocol(ProtocolMessage::HybridPir(HybridPirMessage::Response(resp))) = response {
                        Ok(resp)
                    } else {
                        unreachable!();
                    }
                })
                .with_max_len(1)
                .collect::<Result<Vec<PirReply>, Error>>()?;

            debug!("Response size: {:?}", responses[0].reply.len());

            let t = std::time::Instant::now();
            client.combine(index, responses);
            debug!("Decode time: {:?}", t.elapsed().as_secs_f64() * 1000.0);
        },
    }

    Ok(())
}

fn run_series(streams: &mut Vec<TcpStream>, params: BenchmarkParams, iterations: usize) -> Result<f64, Error> {
    streams
        .par_iter()
        .map(|ref mut stream| {
            let msg = BenchmarkMessage::Setup(params.clone());
            msg.write_to(stream)?;
            BenchmarkMessage::read_from(stream)?;
            Ok(())
        })
        .with_max_len(1)
        .collect::<Result<(), Error>>()?;

    let mut times: Vec<f64> = Vec::with_capacity(iterations);
    for i in 0..iterations {
        if i % QUEUE_SIZE == 0 {
            streams.par_iter()
                .map(|ref mut stream| {
                    let msg = BenchmarkMessage::RefreshQueue;
                    msg.write_to(stream)?;
                    BenchmarkMessage::read_from(stream)?;
                    Ok(())
                })
                .with_max_len(1)
                .collect::<Result<(), Error>>()?;
        }

        let t = Instant::now();
        run_query(streams, &params)?;
        let elapsed = t.elapsed().as_secs_f64() * 1000.0;
        times.push(elapsed);
    }

    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = times.iter().fold(0.0, |sum, val| sum + val) / (iterations as f64);

    debug!("min: {:?}, mean: {:?}, max: {:?}", times[0], mean, times[iterations - 1]);

    Ok(mean)
}

fn run_different_element_counts(streams: &mut Vec<TcpStream>, ns: &Vec<usize>, s: usize, b: usize) {
    info!("n;raidpir;sealpir;hybridpir");

    for n in ns.iter() {
        debug!("{:?}", n);
        //let mut params_raidpir = DEFAULT_RAIDPIR.clone();
        //if let BenchmarkParams::RaidPir{
        //    ref mut db_size,
        //    ref mut element_size,
        //    servers: _,
        //    redundancy: _,
        //    russians: _
        //} = params_raidpir {
        //    *db_size = *n;
        //    *element_size = s;
        //}
        //let time_raidpir = run_series(streams, params_raidpir, N).unwrap();
        let time_raidpir = 0;

        let mut params_sealpir = DEFAULT_SEALPIR.clone();
        if let BenchmarkParams::SealPir{
            ref mut db_size,
            ref mut element_size,
            poly_degree: _,
            log: _,
            d: _
        } = params_sealpir {
            *db_size = *n;
            *element_size = s;
        }
        let time_sealpir = run_series(streams, params_sealpir, N).unwrap();
        //let time_sealpir = 0;

        //if (*n / b) >= 2 * 8 {
        if false {
            let mut params_hybridpir = DEFAULT_HYBRIDPIR.clone();
            if let BenchmarkParams::HybridPir{
                ref mut db_size,
                ref mut element_size,
                raidpir_servers: _,
                raidpir_redundancy: _,
                ref mut raidpir_size,
                raidpir_russians: _,
                sealpir_poly_degree: _,
                sealpir_log: _,
                sealpir_d: _,
            } = params_hybridpir {
                *db_size = *n;
                *element_size = s;
                *raidpir_size = n / b;
            }
            let time_hybridpir = run_series(streams, params_hybridpir, N).unwrap();

            info!("{:?};{:?};{:?};{:?}", n, time_raidpir, time_sealpir, time_hybridpir);
        } else {
            info!("{:?};{:?};{:?};", n, time_raidpir, time_sealpir);
        }
    }
}

#[no_mangle]
pub unsafe extern fn Java_de_tu_1darmstadt_cs_encrypto_hybridpir_RustInterface_benchmarkPEM(
    _env: JNIEnv,
    _: JClass,
) {
    android_log::init("HybridPIR").unwrap();

    let servers: Vec<SocketAddr> = ["130.83.125.167:7000", "130.83.125.168:7001"]
        .iter()
        .map(|x| x.to_socket_addrs().unwrap().next().unwrap())
        .collect();

    debug!("{:?}", servers);

    let mut streams: Vec<TcpStream> = servers
        .par_iter() // Establish connections in parallel
        .map(|target| {
            let stream = TcpStream::connect(target)?;
            stream.set_read_timeout(Some(Duration::from_secs(3600)))?;
            stream.set_write_timeout(Some(Duration::from_secs(3600)))?;
            stream.set_nodelay(true)?;
            Ok(stream)
        })
        .with_max_len(1) // Ensure each iteration gets a thread
        .collect::<Result<Vec<TcpStream>, Error>>()
        .unwrap();

    let ns: Vec<usize> = [10027008, 100007936, 1000013824].to_vec();
    //let ns: Vec<usize> = [1000013824].to_vec();
    run_different_element_counts(&mut streams, &ns, 4, 2048);
}
