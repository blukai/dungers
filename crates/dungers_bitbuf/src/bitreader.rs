use dungers_varint::{zigzag_decode32, zigzag_decode64};

use crate::{Error, Result, EXTRA_MASKS};

pub struct BitReader<'a> {
    data_bits: usize,
    data: &'a [u64],
    cur_bit: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        // make sure that alignment is correct.
        debug_assert!(data.len() % 8 == 0);

        Self {
            data_bits: data.len() << 3,
            // SAFETY: transmuting data into a slice of u64s is safe here because BitReader
            // requires the input data to be 8-byte aligned, which is enforced by the debug_assert
            // above.
            data: unsafe { std::mem::transmute(data) },
            cur_bit: 0,
        }
    }

    #[inline(always)]
    pub fn get_num_bits_left(&self) -> usize {
        self.data_bits - self.cur_bit
    }

    #[inline(always)]
    pub fn get_num_bytes_left(&self) -> usize {
        self.get_num_bits_left() >> 3
    }

    #[inline(always)]
    pub fn get_num_bits_read(&self) -> usize {
        self.cur_bit
    }

    #[inline(always)]
    pub fn get_num_bytes_read(&self) -> usize {
        (self.cur_bit + 7) >> 3
    }

    /// seek to a specific bit.
    pub fn seek(&mut self, bit: usize) -> Result<()> {
        if bit > self.data_bits {
            return Err(Error::Overflow);
        }
        self.cur_bit = bit;
        Ok(())
    }

    /// seek to an offset from the current position.
    pub fn seek_relative(&mut self, bit_delta: isize) -> Result<usize> {
        let bit = self.cur_bit as isize + bit_delta;
        if bit < 0 {
            return Err(Error::Overflow);
        }
        self.seek(bit as usize)?;
        Ok(self.cur_bit)
    }

    #[inline]
    pub fn read_ubit64(&mut self, num_bits: usize) -> Result<u64> {
        // make sure that the requested number of bits to read is in bounds of u64.
        debug_assert!(num_bits <= 64);

        if self.get_num_bits_left() < num_bits {
            return Err(Error::Overflow);
        }

        // SAFETY: assert and check above ensure that we'll not go out of bounds.

        let block1_idx = self.cur_bit >> 6;

        let mut block1 = *unsafe { self.data.get_unchecked(block1_idx) };
        // get the bits we're interested in
        block1 >>= self.cur_bit & 63;

        self.cur_bit += num_bits;
        let mut ret = block1;

        // does it span this block?
        if (self.cur_bit - 1) >> 6 == block1_idx {
            ret &= EXTRA_MASKS[num_bits];
        } else {
            let extra_bits = self.cur_bit & 63;

            let mut block2 = *unsafe { self.data.get_unchecked(block1_idx + 1) };
            block2 &= EXTRA_MASKS[extra_bits];

            // no need to mask since we hit the end of the dword.
            // shift the second dword's part into the high bits.
            ret |= block2 << (num_bits - extra_bits);
        }

        Ok(ret)
    }

    // int old_bf_read::ReadByte()
    pub fn read_u8(&mut self) -> Result<u8> {
        self.read_ubit64(8).map(|result| result as u8)
    }

    // NOTE: ref impl for varints:
    // https://github.com/rust-lang/rust/blob/e5b3e68abf170556b9d56c6f9028318e53c9f06b/compiler/rustc_serialize/src/leb128.rs

    // TODO: this can be faster
    //
    // uint64 old_bf_read::ReadVarInt64()
    pub fn read_uvarint64(&mut self) -> Result<u64> {
        let byte = self.read_u8()?;
        if (byte & 0x80) == 0 {
            return Ok(byte as u64);
        }
        let mut result = (byte & 0x7f) as u64;
        for count in 1..=10 {
            let byte = self.read_u8()?;
            if (byte & 0x80) == 0 {
                result |= (byte as u64) << (count * 7);
                return Ok(result);
            }
            result |= ((byte & 0x7f) as u64) << (count * 7);
        }
        Err(Error::MalformedVarint)
    }

    // TODO: this can be faster
    //
    // uint32 old_bf_read::ReadVarInt32()
    pub fn read_uvarint32(&mut self) -> Result<u32> {
        let byte = self.read_u8()?;
        if (byte & 0x80) == 0 {
            return Ok(byte as u32);
        }
        let mut result = (byte & 0x7f) as u32;
        for count in 1..=5 {
            let byte = self.read_u8()?;
            if (byte & 0x80) == 0 {
                result |= (byte as u32) << (count * 7);
                return Ok(result);
            }
            result |= ((byte & 0x7f) as u32) << (count * 7);
        }
        Err(Error::MalformedVarint)
    }

    // int32 ReadSignedVarInt32()
    pub fn read_varint64(&mut self) -> Result<i64> {
        self.read_uvarint64().map(zigzag_decode64)
    }

    // int64 ReadSignedVarInt64()
    pub fn read_varint32(&mut self) -> Result<i32> {
        self.read_uvarint32().map(zigzag_decode32)
    }

    #[inline(always)]
    pub fn read_bool(&mut self) -> Result<bool> {
        if self.get_num_bits_left() < 1 {
            return Err(Error::Overflow);
        }

        // SAFETY: check above ensures that we'll not go out of bounds.
        let one_bit =
            unsafe { self.data.get_unchecked(self.cur_bit >> 6) } >> (self.cur_bit & 63) & 1;
        self.cur_bit += 1;

        Ok(one_bit == 1)
    }
}
