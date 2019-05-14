
use nom::character::complete::{hex_digit1, digit1, space0, alphanumeric1};

#[cfg(test)]
mod tests {
    use crate::candump_parser::*;

    #[test]
    fn it_works() {
        let exp = DumpEntry {
            timestamp: Timestamp { seconds: 1547046014, nanos: 597158 },
            can_interface: "vcan0".to_string(),
            can_frame: CanFrame {
                frame_id: 123,
                frame_body: 455,
            }
        };
       assert_eq!(dump_entry("(1547046014.597158) vcan0 7B#1C7"), Ok(("", exp)));
    }
}

#[derive(Debug, PartialEq)]
pub struct Timestamp {
    pub seconds: u64,
    pub nanos: u64,
}

named!(timestamp<&str, Timestamp>,
    do_parse!(
                 tag!("(")                             >>
        seconds: map_res!(digit1, |d: &str| d.parse()) >>
                 tag!(".")                             >>
        nanos:   map_res!(digit1, |d: &str| d.parse()) >>
                 tag!(")")                             >>
        (Timestamp { seconds, nanos })
    )
);

#[derive(Debug, PartialEq)]
pub struct CanFrame {
    pub frame_id: u32,
    pub frame_body: u64,
}

named!(can_frame<&str, CanFrame>,
    do_parse!(
        frame_id:   map_res!(hex_digit1, |d| u32::from_str_radix(d, 16))  >>
                    tag!("#")                                             >>
        frame_body: map_res!(hex_digit1, |d| u64::from_str_radix(d, 16))  >>
        (CanFrame { frame_id, frame_body })
    )
);

#[derive(Debug, PartialEq)]
pub struct DumpEntry {
    timestamp: Timestamp,
    can_interface: String,
    can_frame: CanFrame,
}

impl DumpEntry {
    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }

    pub fn can_interface(&self) -> &str {
        &self.can_interface
    }

    pub fn can_frame(&self) -> &CanFrame {
        &self.can_frame
    }
}

named!(pub dump_entry<&str, DumpEntry>,
    do_parse!(
        timestamp:     timestamp     >>
                       space0        >>
        can_interface: alphanumeric1 >>
                       space0        >>
        can_frame:     can_frame     >>
        (DumpEntry { timestamp, can_interface: can_interface.to_string(), can_frame })
    )
);