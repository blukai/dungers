use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error")]
    Io(#[from] io::Error),
    #[error("malformed varint")]
    MalformedVarint,
}

pub type Result<T> = std::result::Result<T, Error>;

// ZigZag Transform:  Encodes signed integers so that they can be effectively
// used with varint encoding.
//
// varint operates on unsigned integers, encoding smaller numbers into fewer
// bytes.  If you try to use it on a signed integer, it will treat this number
// as a very large unsigned integer, which means that even small signed numbers
// like -1 will take the maximum number of bytes (10) to encode.  ZigZagEncode()
// maps signed integers to unsigned in such a way that those with a small
// absolute value will have smaller encoded values, making them appropriate for
// encoding using varint.
//
//       int32 ->     uint32
// -------------------------
//           0 ->          0
//          -1 ->          1
//           1 ->          2
//          -2 ->          3
//         ... ->        ...
//  2147483647 -> 4294967294 -2147483648 -> 4294967295
//
//        >> encode >>
//        << decode <<

#[inline(always)]
pub fn zigzag_encode32(n: i32) -> u32 {
    ((n << 1) ^ (n >> 31)) as u32
}

#[inline(always)]
pub fn zigzag_encode64(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

#[inline(always)]
pub fn zigzag_decode32(n: u32) -> i32 {
    (n >> 1) as i32 ^ -((n & 1) as i32)
}

#[inline(always)]
pub fn zigzag_decode64(n: u64) -> i64 {
    (n >> 1) as i64 ^ -((n & 1) as i64)
}

// Each byte in the varint has a continuation bit that indicates if the byte
// that follows it is part of the varint. This is the most significant bit (MSB)
// of the byte. The lower 7 bits are a payload; the resulting integer is built
// by appending together the 7-bit payloads of its constituent bytes.
// This allows variable size numbers to be stored with tolerable
// efficiency. Numbers sizes that can be stored for various numbers of
// encoded bits are:
//  8-bits: 0-127
// 16-bits: 128-16383
// 24-bits: 16384-2097151
// 32-bits: 2097152-268435455
// 40-bits: 268435456-0xFFFFFFFF

macro_rules! impl_write_uvarint {
    ($fn_name:ident, $ty:ty) => {
        #[inline]
        pub fn $fn_name<W: io::Write>(mut w: W, value: $ty) -> Result<usize> {
            let mut value = value;
            let mut buf = [0u8; 1];
            let mut count = 0;
            loop {
                if value < 0x80 {
                    *unsafe { buf.get_unchecked_mut(0) } = value as u8;
                    w.write_all(&buf)?;
                    count += 1;
                    break;
                }

                *unsafe { buf.get_unchecked_mut(0) } = ((value & 0x7f) | 0x80) as u8;
                w.write_all(&buf)?;
                value >>= 7;
                count += 1;
            }
            Ok(count)
        }
    };
}

impl_write_uvarint!(write_uvarint32, u32);
impl_write_uvarint!(write_uvarint64, u64);

#[inline]
pub fn write_varint32<W: io::Write>(w: W, value: i32) -> Result<usize> {
    write_uvarint32(w, zigzag_encode32(value))
}

#[inline]
pub fn write_varint64<W: io::Write>(w: W, value: i64) -> Result<usize> {
    write_uvarint64(w, zigzag_encode64(value))
}

/// returns the max size (in bytes) of varint encoded number for `T`, assuming `T` is an integer
/// type.
const fn max_varint_size<T>() -> usize {
    // The longest varint encoding for an integer uses 7 bits per byte.
    (std::mem::size_of::<T>() * 8 + 6) / 7
}

macro_rules! impl_read_uvarint {
    ($fn_name:ident, $ty:ty) => {
        #[inline]
        pub fn $fn_name<R: io::Read>(r: R) -> Result<($ty, usize)> {
            let mut bytes = r.bytes();

            let Some(byte) = bytes.next().transpose()? else {
                return Err(Error::Io(io::Error::from(io::ErrorKind::UnexpectedEof)));
            };
            if (byte & 0x80) == 0 {
                return Ok((byte as $ty, 1));
            }

            let mut result = (byte & 0x7f) as $ty;

            for count in 1..=max_varint_size::<$ty>() {
                let Some(byte) = bytes.next().transpose()? else {
                    return Err(Error::Io(io::Error::from(io::ErrorKind::UnexpectedEof)));
                };
                if (byte & 0x80) == 0 {
                    result |= (byte as $ty) << (count * 7);
                    return Ok((result, count));
                }

                result |= ((byte & 0x7f) as $ty) << (count * 7);
            }

            Err(Error::MalformedVarint)
        }
    };
}

impl_read_uvarint!(read_uvarint32, u32);
impl_read_uvarint!(read_uvarint64, u64);

#[inline]
pub fn read_varint32<R: io::Read>(r: R) -> Result<(i32, usize)> {
    read_uvarint32(r).map(|(value, n)| (zigzag_decode32(value), n))
}

#[inline]
pub fn read_varint64<R: io::Read>(r: R) -> Result<(i64, usize)> {
    read_uvarint64(r).map(|(value, n)| (zigzag_decode64(value), n))
}
