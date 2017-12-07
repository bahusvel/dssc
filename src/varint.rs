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

pub fn uvarint(buf: &[u8]) -> (u64, usize) {
    let mut x = 0u64;
    let mut s = 0usize;
    for i in 0..buf.len() {
        let b = buf[i];
        if b < 0x80 {
            if i > 9 || i == 9 && b > 1 {
                return (0u64, <usize>::max_value() - (i + 1)); // overflow
            }
            return (x | (b as u64) << s, (i + 1) as usize);
        }
        x |= ((b & 0x7f) as u64) << s;
        s += 7;
    }
    (0u64, 0usize)
}
