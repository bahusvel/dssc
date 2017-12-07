extern crate dssc;
extern crate byteorder;

use dssc::{DSSCDecoder, DSSCEncoder};
use std::env;
use std::io::{stdin, stdout, Read, Write, Stdin};
use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};

fn main() {
    if let Some(d) = env::args().nth(1) {
        if d != "-d".to_string() {
            println!("Usage: linedssc [-d] (d means decompress)");
            return;
        }
        let mut decoder = DSSCDecoder::new();
        loop {
            let len = stdin().read_u32::<BigEndian>().expect("Unexpected input");
            let mut buf = Vec::new();
            if let Err(error) = stdin().take(len as u64).read_to_end(&mut buf) {
                println!("error: {}", error);
                return;
            }
            stdout().write_all(&decoder.decode(&buf)).expect(
                "write failed",
            )
        }
    }
    let mut encoder = DSSCEncoder::new();
    loop {
        let mut input = String::new();
        if let Err(error) = stdin().read_line(&mut input) {
            println!("error: {}", error);
            return;
        }
        let encoded = encoder.encode(input.as_bytes());
        stdout()
            .write_u32::<BigEndian>(encoded.len() as u32)
            .unwrap();
        stdout().write(&encoded).expect("write failed");
    }
}
