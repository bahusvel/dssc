use std::io::{Read, Error, ErrorKind};
// these are roughly ported from https://golang.org/src/encoding/binary/varint.go

pub fn put_uvarint(buf: &mut [u8], mut x: u64) -> usize {
    let mut i = 0;
    while x >= 0x80 {;
        buf[i] = x as u8 | 0x80;
        x >>= 7;
        i += 1;
    }
    buf[i] = x as u8;
    return i + 1;
}

pub fn uvarint(buf: &[u8]) -> (u64, isize) {
    let mut x = 0u64;
    let mut s = 0isize;
    for i in 0..buf.len() {
        let b = buf[i];
        if b < 0x80 {
            if i > 9 || i == 9 && b > 1 {
                return (0u64, -1); // overflow
            }
            return (x | (b as u64) << s, (i + 1) as isize);
        }
        x |= ((b & 0x7f) as u64) << s;
        s += 7;
    }
    (0u64, 0isize)
}

pub fn read_uvarint(r: &mut Read) -> Result<u64, Error> {
    let mut x = 0u64;
    let mut s = 0isize;
    let mut i = 0;
    let mut b = [0; 1];
    loop {
        r.read(&mut b)?;
        if b[0] < 0x80 {
            if i > 9 || i == 9 && b[0] > 1 {
                return Err(Error::new(ErrorKind::Other, "Overflow")); // overflow
            }
            return Ok(x | (b[0] as u64) << s);
        }
        x |= ((b[0] & 0x7f) as u64) << s;
        s += 7;
        i += 1;
    }
    Ok(0)
}

#[test]
pub fn varint_test() {
    let mut buf = [0; 9];
    let val = 10000;
    println!("{}", put_uvarint(&mut buf, val));
    let (after, size) = uvarint(&buf);
    println!("{} {}", after, size);
    assert_eq!(val, after);
}
