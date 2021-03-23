use core::fmt::Display;
use futures::prelude::*;
use futures_util::compat::Stream01CompatExt;
use std::{collections::HashMap, fmt};
use structopt::StructOpt;
use tokio_socketcan;

#[derive(Debug, StructOpt)]
#[structopt(name = "canstatsrb", about = "SocketCAN message statistics")]
struct Opt {
    /// Set can interface
    #[structopt(help = "socketcan CAN interface e.g. vcan0")]
    can_interface: String,
}

#[derive(Default, Debug)]
struct Stats {
    msg_ids: HashMap<u32, u64>,
    rx_frames: u64,
    eff_frames_total: u64,
    eff_frames_err: u64,
    eff_frames_rtr: u64,
    sff_frames_total: u64,
    sff_frames_err: u64,
    sff_frames_rtr: u64,
}

impl Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "RX Total: {}", self.rx_frames)?;

        write!(f, "EFF Total: {}\t", self.eff_frames_total)?;
        write!(f, "ERR: {}\t", self.eff_frames_err)?;
        writeln!(f, "RTR: {}", self.eff_frames_rtr)?;

        write!(f, "SFF Total: {}\t", self.sff_frames_total)?;
        write!(f, "ERR: {}\t", self.sff_frames_err)?;
        writeln!(f, "RTR: {}", self.sff_frames_rtr)?;
        writeln!(f, "Messages by CAN ID")?;

        let mut count_vec: Vec<(&u32, &u64)> = self.msg_ids.iter().collect();
        count_vec.sort_by(|a, b| a.0.cmp(b.0));
        for (ref id, ref count) in &count_vec {
            writeln!(f, "{: ^10} → #{: ^7}", id, count)?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let opt = Opt::from_args();

    let mut socket_rx = tokio_socketcan::CANSocket::open(&opt.can_interface)
        .unwrap()
        .compat();

    let mut stats: Stats = Default::default();

    while let Some(socket_result) = socket_rx.next().await {
        match socket_result {
            Ok(frame) => {
                stats.rx_frames += 1;

                if frame.is_extended() {
                    stats.sff_frames_total += 1;

                    if frame.is_error() {
                        stats.sff_frames_err += 1;
                    }

                    if frame.is_rtr() {
                        stats.sff_frames_rtr += 1;
                    }
                } else {
                    stats.eff_frames_total += 1;

                    if frame.is_error() {
                        stats.eff_frames_err += 1;
                    }

                    if frame.is_rtr() {
                        stats.eff_frames_rtr += 1;
                    }
                }

                stats
                    .msg_ids
                    .entry(frame.id())
                    .and_modify(|e| *e += 1)
                    .or_insert(1);

                println!("{}", stats);
            }
            Err(err) => {
                eprintln!("IO error: {}", err);
            }
        }
    }

    Ok(())
}
