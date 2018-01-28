extern crate byteorder;
extern crate clap;
extern crate dssc;

use dssc::Compressor;
use dssc::chunked::ChunkedCompressor;
use dssc::chunkmap::ChunkMap;
use dssc::flate::FlateCompressor;
use dssc::varint::{put_uvarint, read_uvarint};
use std::ops::DerefMut;
use std::io::{stdin, stdout, BufRead, BufReader, Error, Read, Write};
use clap::{App, Arg};
use std::fs::File;

const DEFAULT_THRESHOLD: f32 = 0.5;

fn encode<R: Read, W: Write>(comp: &mut Compressor, input: R, mut output: W) -> Result<(), Error> {
    let mut len_buf = [0; 10];
    let mut reader = BufReader::new(input);
    loop {
        let mut ibuf = String::new();
        let n = reader.read_line(&mut ibuf)?;
        if n == 0 {
            return Ok(());
        }
        let mut encoded = Vec::new();
        comp.encode(ibuf.as_bytes(), &mut encoded);
        let len_len = put_uvarint(&mut len_buf, encoded.len() as u64);
        output.write(&len_buf[0..len_len])?;
        output.write(&encoded)?;
    }
}

fn decode<R: Read, W: Write>(
    comp: &mut Compressor,
    mut input: R,
    mut output: W,
) -> Result<(), Error> {
    loop {
        let mut buf = Vec::new();
        let len = read_uvarint(&mut input)?;
        let n = stdin().take(len as u64).read_to_end(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        let mut decoded = Vec::new();
        comp.decode(&buf, &mut decoded);
        output.write_all(&decoded)?;
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
                .possible_values(&["chunkmap", "chunked", "flate"])
                .default_value("chunkmap")
                .help("Switches linedssc to use a different algorithm")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("input")
                .default_value("-")
                .required(true)
                .help("File like source to take the lines from"),
        )
        .arg(
            Arg::with_name("output")
                .default_value("-")
                .required(true)
                .help("File like destination to output compressed data"),
        )
        .get_matches();

    let threshold = matches
        .value_of("threshold")
        .map(|t| t.parse().expect("Incorrect format for threshold"))
        .unwrap_or(DEFAULT_THRESHOLD);

    let mut comp: Box<Compressor> = match matches.value_of("algorithm") {
        Some("chunkmap") => Box::new(ChunkMap::new(threshold)),
        Some("chunked") => Box::new(ChunkedCompressor::new(threshold)),
        Some("flate") => Box::new(FlateCompressor::default()),
        Some(_) | None => panic!("Cannot be none"),
    };

    let input: Box<Read> = match matches.value_of("input") {
        Some("-") => Box::new(stdin()),
        Some(file) => Box::new(File::open(file).expect("Could not open input")),
        _ => panic!("This is not supposed to happen"),
    };

    let output: Box<Write> = match matches.value_of("output") {
        Some("-") => Box::new(stdout()),
        Some(file) => Box::new(File::create(file).expect("Could not open ouput")),
        _ => panic!("This is not supposed to happen"),
    };

    if matches.is_present("decompress") {
        if let Err(error) = decode(comp.deref_mut(), input, output) {
            eprintln!("error: {}", error);
        }
    } else {
        if let Err(error) = encode(comp.deref_mut(), input, output) {
            eprintln!("error: {}", error);
        }
    }
}
