extern crate dssc;
extern crate byteorder;

use dssc::{DSSCDecoder, DSSCEncoder};
use dssc::varint::{put_uvarint, read_uvarint};
use std::env;
use std::io::{stdin, stdout, Read, Write, Error};

fn encode() -> Result<(), Error> {
    let mut encoder = DSSCEncoder::new();
    let mut len_buf = [0; 10];
    loop {
        let mut input = String::new();
        let n = stdin().read_line(&mut input)?;
        if n == 0 {
            return Ok(());
        }
        let encoded = encoder.encode(input.as_bytes());
        let len_len = put_uvarint(&mut len_buf, encoded.len() as u64);
        stdout().write(&len_buf[0..len_len])?;
        stdout().write(&encoded)?;
    }
}

fn decode() -> Result<(), Error> {
    let mut decoder = DSSCDecoder::new();
    loop {
        let mut buf = Vec::new();
        let len = read_uvarint(&mut stdin())?;
        let n = stdin().take(len as u64).read_to_end(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        stdout().write_all(&decoder.decode(&buf))?;
    }
}

fn main() {
    if let Some(d) = env::args().nth(1) {
        if d != "-d".to_string() {
            eprintln!("Usage: linedssc [-d] (d means decompress)");
            return;
        }
        if let Err(error) = decode() {
            eprintln!("error: {}", error);
        }
    }
    if let Err(error) = encode() {
        eprintln!("error: {}", error);
    }
}
