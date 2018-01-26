extern crate dssc;
extern crate byteorder;
extern crate clap;

use dssc::Compressor;
use dssc::chunked::ChunkedCompressor;
use dssc::chunkmap::ChunkMap;
use dssc::flate::FlateCompressor;
use dssc::varint::{put_uvarint, read_uvarint};
use std::env;
use std::ops::DerefMut;
use std::io::{stdin, stdout, Read, Write, Error};
use clap::{Arg, App};

const DEFAULT_THRESHOLD: f32 = 0.5;

fn encode(comp: &mut Compressor) -> Result<(), Error> {
    let mut len_buf = [0; 10];
    loop {
        let mut input = String::new();
        let n = stdin().read_line(&mut input)?;
        if n == 0 {
            return Ok(());
        }
        let encoded = comp.encode(input.as_bytes());
        let len_len = put_uvarint(&mut len_buf, encoded.len() as u64);
        stdout().write(&len_buf[0..len_len])?;
        stdout().write(&encoded)?;
    }
}

fn decode(comp: &mut Compressor) -> Result<(), Error> {
    loop {
        let mut buf = Vec::new();
        let len = read_uvarint(&mut stdin())?;
        let n = stdin().take(len as u64).read_to_end(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        stdout().write_all(&comp.decode(&buf))?;
    }
}

fn main() {
    let matches = App::new("Linefed Discrete Stream Compressor")
        .version("0.0")
        .author("Denis Lavrov <bahus.vel@gmail.com>")
        .about("Compresses stream of lines")
        .arg(
            Arg::with_name("threshold")
                .short("t")
                .long("threshold")
                .help("Sets insert threshold for history cache")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("decompress")
                .short("d")
                .long("decompress")
                .help("Switches linedssc into decompress mode"),
        )
        .arg(
            Arg::with_name("algorithm")
                .short("a")
                .long("algorithm")
                .possible_values(&["convolve", "chunked", "flate"])
                .default_value("chunked")
                .help("Switches linedssc to use a different algorithm")
                .takes_value(true),
        )
        .get_matches();

    let threshold = matches
        .value_of("threshold")
        .map(|t| t.parse().expect("Incorrect format for threshold"))
        .unwrap_or(DEFAULT_THRESHOLD);

    let mut comp: Box<Compressor> = match matches.value_of("algorithm") {
        Some("chunkmap") => Box::new(ChunkMap::new()),
        Some("chunked") => Box::new(ChunkedCompressor::new(threshold)),
        Some("flate") => Box::new(FlateCompressor::default()),
        Some(_) | None => panic!("Cannot be none"),
    };


    if matches.is_present("decompress") {
        if let Err(error) = decode(comp.deref_mut()) {
            eprintln!("error: {}", error);
        }
    } else {
        if let Err(error) = encode(comp.deref_mut()) {
            eprintln!("error: {}", error);
        }
    }
}
