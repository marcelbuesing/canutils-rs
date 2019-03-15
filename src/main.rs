
use ansi_term::Color;
use ansi_term::Color::Fixed;

use futures::stream::Stream;
use futures::future::{self, Future};

use std::fmt::Write;
use std::io::{self};
use std::path::PathBuf;

use structopt::StructOpt;

use tokio::prelude::*;
use tokio::io::stdout;
use tokio_socketcan;

const COLOR_CAN_ID: Color = Color::White;
const COLOR_CAN_SFF: Color = Color::Blue;
const COLOR_CAN_EFF: Color = Color::Red;

const COLOR_NULL: Color = Fixed(242); // grey
const COLOR_OFFSET: Color = Fixed(242); // grey
const COLOR_ASCII_PRINTABLE: Color = Color::Cyan;
const COLOR_ASCII_WHITESPACE: Color = Color::Green;
const COLOR_ASCII_OTHER: Color = Color::Purple;
const COLOR_NONASCII: Color = Color::Yellow;

enum ByteCategory {
    Null,
    AsciiPrintable,
    AsciiWhitespace,
    AsciiOther,
    NonAscii,
}

#[derive(Copy, Clone)]
struct Byte(u8);

impl Byte {
    fn category(self) -> ByteCategory {
        if self.0 == 0x00 {
            ByteCategory::Null
        } else if self.0.is_ascii_alphanumeric()
            || self.0.is_ascii_punctuation()
            || self.0.is_ascii_graphic()
        {
            ByteCategory::AsciiPrintable
        } else if self.0.is_ascii_whitespace() {
            ByteCategory::AsciiWhitespace
        } else if self.0.is_ascii() {
            ByteCategory::AsciiOther
        } else {
            ByteCategory::NonAscii
        }
    }

    fn color(self) -> &'static Color {
        use ByteCategory::*;

        match self.category() {
            Null => &COLOR_NULL,
            AsciiPrintable => &COLOR_ASCII_PRINTABLE,
            AsciiWhitespace => &COLOR_ASCII_WHITESPACE,
            AsciiOther => &COLOR_ASCII_OTHER,
            NonAscii => &COLOR_NONASCII,
        }
    }

    fn as_char(self) -> char {
        use ByteCategory::*;

        match self.category() {
            Null => '0',
            AsciiPrintable => self.0 as char,
            AsciiWhitespace if self.0 == 0x20 => ' ',
            AsciiWhitespace => '_',
            AsciiOther => '•',
            NonAscii => '×',
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "candumprb", about = "Candump Rainbow. A colorful can dump tool with dbc support.")]
struct Opt {
    /// DBC Input file
    #[structopt(short = "f", long = "dbc-file", parse(from_os_str))]
    dbc_file: Option<PathBuf>,

    /// Set can interface
    #[structopt(help = "socketcan CAN interface e.g. vcan0")]
    can_interface: String,
}

fn main() -> io::Result<()> {

    let opt = Opt::from_args();

    let socket_rx = tokio_socketcan::CANSocket::open(&opt.can_interface).unwrap();

    let byte_hex_table: Vec<String> = (0u8..=u8::max_value())
        .map(|i| {
            let byte_hex = format!("{:02x} ", i);
            Byte(i).color().paint(byte_hex).to_string()
        })
    .collect();

    tokio::run(socket_rx.for_each(move |frame| {
        let mut buffer: String = String::new();

        if frame.is_extended() {
            write!(buffer, "{}", COLOR_CAN_EFF.paint("EFF ")).unwrap();
        } else {
            write!(buffer, "{}", COLOR_CAN_SFF.paint("SFF ")).unwrap();
        }

        write!(buffer, "{}\t", COLOR_CAN_ID.paint(format!("{:X}", frame.id()))).unwrap();

        for b in frame.data() {
            write!(buffer, "{}", byte_hex_table[*b as usize]).unwrap();
        }

        println!("{}", buffer);

        Ok(())
    }).map_err(|_err| {}));

    Ok(())
}
