use ansi_term::{
    Color::Red,
    Colour::{Blue, Cyan, Green, Purple},
};
use can_dbc::{self, Message};
use futures_timer::Delay;
use futures_util::compat::Future01CompatExt;
use pretty_hex::*;
use rand::Rng;
use socketcan;
use std::{
    fs::File,
    io::{self, prelude::*},
    path::PathBuf,
    time::{Duration, Instant},
};
use structopt::StructOpt;
use tokio_socketcan::CANFrame;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cangenrb",
    about = "Cangen Rainbow. A colorful that generates CAN messages based on a supplied DBC file."
)]
struct Opt {
    /// Completely random frame data, unrelated to any signal
    #[structopt(short = "r", long = "random-frame-data")]
    random_frame_data: bool,

    /// DBC file path
    #[structopt(short = "i", long = "input", parse(from_os_str))]
    input: PathBuf,

    /// Send random remote transmission frames
    #[structopt(long = "rtr")]
    rtr_frames: bool,

    /// Send random error frames
    #[structopt(long = "err")]
    err_frames: bool,

    /// Frequency of sending in microseconds
    #[structopt(short = "f", long = "frequency", default_value = "100000")]
    frequency: u64,

    /// Only generate messages of the given transmitter (sending node)
    #[structopt(long = "transmitter")]
    transmitter: Option<String>,

    /// Only generate messages for the given receiver (receiving node)
    //#[structopt(long = "receiver")]
    //receiver: Option<String>,

    #[structopt(name = "CAN_INTERFACE")]
    can_interface: String,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let opt = Opt::from_args();

    let mut f = File::open(&opt.input)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;

    let socket_tx = tokio_socketcan::CANSocket::open(&opt.can_interface).unwrap();
    let dbc = can_dbc::DBC::from_slice(&buffer).expect("Failed to parse DBC");

    // Filter messages by transmitter
    let dbc_messages = if let Some(transmitter) = opt.transmitter {
        let expected_transmitter = can_dbc::Transmitter::NodeName(transmitter);
        dbc.messages()
            .iter()
            .filter(|m| *m.transmitter() == expected_transmitter)
            .map(|m| m.to_owned())
            .collect()
    } else {
        dbc.messages().clone()
    };

    // Create signal generators
    let dbc_signal_range_gen = DBCSignalRangeGen {
        dbc_messages: dbc_messages.clone(),
        err_frames: opt.err_frames,
        rtr_frames: opt.rtr_frames,
    };
    let random_frame_data_gen = RandomFrameDataGen {
        dbc_messages: dbc_messages.clone(),
        err_frames: opt.err_frames,
        rtr_frames: opt.rtr_frames,
    };

    let mut messages_sent_counter: u128 = 0;

    let now = Instant::now();
    loop {
        Delay::new(Duration::from_micros(opt.frequency)).await;
        let can_frame = if opt.random_frame_data {
            random_frame_data_gen.gen()
        } else {
            dbc_signal_range_gen.gen()
        };

        socket_tx.write_frame(can_frame).compat().await?;

        messages_sent_counter += 1;

        let message_througput = messages_sent_counter as f64 / now.elapsed().as_secs() as f64;

        println!(
            "✉ #{}  ✉ {:.2}msgs/s  ⧖ {}ms",
            messages_sent_counter,
            message_througput,
            now.elapsed().as_millis()
        );
    }
}

trait CanFrameGenStrategy: Send {
    fn gen(&self) -> CANFrame;
}

/// Generates signal values based on a given DBC messages.
/// Signals offset and width is based on the DBC.
/// Random signal values are generated within the range that is specified in the DBC.
struct DBCSignalRangeGen {
    dbc_messages: Vec<Message>,
    rtr_frames: bool,
    err_frames: bool,
}

impl CanFrameGenStrategy for DBCSignalRangeGen {
    fn gen(&self) -> CANFrame {
        let mut rng = rand::thread_rng();
        let message_idx: usize = rng.gen_range(0, self.dbc_messages.len() - 1);
        let message = self.dbc_messages.get(message_idx).unwrap();

        println!("\n{}", Purple.paint(message.message_name()));
        let rtr_rand = if self.rtr_frames {
            rand::random()
        } else {
            false
        };

        let err_rand = if self.err_frames {
            rand::random()
        } else {
            false
        };

        let rand_frame_data = if *message.message_size() > 8 {
            println!("Non random message body due to currently unsupported size `{}` - id: `{:x}`. Size {} > 8", message.message_name(), message.message_id().0, message.message_size());
            [0; 8]
        } else {
            self.gen_msg_frame_data(&message)
        };

        println!(
            "→ ERR: {} RTR: {} Data: {}",
            err_rand,
            rtr_rand,
            rand_frame_data.to_vec().hex_dump()
        );

        let message_id = message.message_id().0 & socketcan::EFF_MASK;

        CANFrame::new(message_id, &rand_frame_data, rtr_rand, err_rand)
            .expect("Failed to create frame")
    }
}

impl DBCSignalRangeGen {
    fn gen_msg_frame_data(&self, message: &can_dbc::Message) -> [u8; 8] {
        let mut frame_data_rand: u64 = 0;

        let mut rng = rand::thread_rng();

        for signal in message.signals() {
            let actual_value: f64 = if signal.min() == signal.max() {
                println!(
                    "Min and max value `{} = {}` match for signal {}, can not create random value.",
                    signal.min(),
                    signal.max(),
                    signal.name()
                );
                *signal.min()
            } else {
                rng.gen_range(signal.min(), signal.max())
            };

            let random_signal_value = (actual_value - signal.offset) / signal.factor;
            let bit_mask: u64 = 2u64.pow(*signal.signal_size() as u32) - 1;

            let min_s = format!("{}", signal.min());
            let max_s = format!("{}", signal.max());
            // let actual_value_s = format!("{:6.4}", (actual_value as u64 & bit_mask) as f64);
            let actual_value_s = format!("{:6.4}", actual_value);

            println!(
                "{:10.10} min {:6.4} max {:6.4} → value {}",
                Green.paint(signal.name()),
                Blue.paint(min_s),
                Red.paint(max_s),
                Cyan.paint(actual_value_s),
            );

            assert!(actual_value >= *signal.min());
            assert!(actual_value <= *signal.max());

            let shifted_signal_value =
                (random_signal_value as u64 & bit_mask) << (signal.start_bit as u8);

            frame_data_rand |= shifted_signal_value;
        }

        frame_data_rand.to_le_bytes()
    }
}

/// Generate random frame payloads. The generated payload does not conform to the DBC.
/// DBC message ids are based on the given DBC.
struct RandomFrameDataGen {
    dbc_messages: Vec<Message>,
    rtr_frames: bool,
    err_frames: bool,
}

impl CanFrameGenStrategy for RandomFrameDataGen {
    fn gen(&self) -> CANFrame {
        let mut rng = rand::thread_rng();
        let message_idx: usize = rng.gen_range(0, self.dbc_messages.len() - 1);
        let message = self.dbc_messages.get(message_idx).unwrap();

        println!("\n{}", Purple.paint(message.message_name()));
        let rtr_rand = if self.rtr_frames {
            rand::random()
        } else {
            false
        };

        let err_rand = if self.err_frames {
            rand::random()
        } else {
            false
        };

        let mut rand_frame_data: [u8; 8] = [0; 8];
        rng.fill(&mut rand_frame_data[..]);

        println!(
            "→ ERR: {} RTR: {} Data: {}",
            err_rand,
            rtr_rand,
            rand_frame_data.to_vec().hex_dump()
        );

        let message_id = message.message_id().0 & socketcan::EFF_MASK;
        CANFrame::new(message_id, &rand_frame_data, rtr_rand, err_rand)
            .expect("Failed to create frame")
    }
}
