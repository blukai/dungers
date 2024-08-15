use dungers_varint::{zigzag_encode32, zigzag_encode64};

use crate::{Error, Result, BIT_WRITE_MASKS, EXTRA_MASKS};

pub struct BitWriter<'a> {
    data_bits: usize,
    data: &'a mut [u64],
    cur_bit: usize,
}

impl<'a> BitWriter<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        // make sure that alignment is correct.
        debug_assert!(buf.len() % 8 == 0);

        Self {
            data_bits: buf.len() << 3,
            // SAFETY: transmuting data into a slice of u64s is safe here because BitWriter
            // requires the input data to be 8-byte aligned, which is enforced by the debug_assert
            // above.
            data: unsafe { std::mem::transmute(&mut *buf) },
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
    pub fn get_num_bits_written(&self) -> usize {
        self.cur_bit
    }

    #[inline(always)]
    pub fn get_num_bytes_written(&self) -> usize {
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
    pub fn write_ubit64(&mut self, data: u64, n: usize) -> Result<()> {
        // make sure that the requested number of bits to write is in bounds of u64.
        debug_assert!(n <= 64);

        if self.cur_bit + n > self.data_bits {
            return Err(Error::Overflow);
        }

        // erase bits at n and higher positions
        let data = data & EXTRA_MASKS[n];

        let block1_idx = self.cur_bit >> 6;
        let bit_offset = self.cur_bit & 63;

        // SAFETY: assert and check above ensure that we'll not go out of bounds.

        let mut block1 = *unsafe { self.data.get_unchecked(block1_idx) };
        block1 &= BIT_WRITE_MASKS[bit_offset][n];
        block1 |= data << bit_offset;
        *unsafe { self.data.get_unchecked_mut(block1_idx) } = block1;

        // did it span a block?
        let bits_written = 64 - bit_offset;
        if bits_written < n {
            let data = data >> bits_written;
            let n = n - bits_written;

            let block2_idx = block1_idx + 1;

            let mut block2 = *unsafe { self.data.get_unchecked(block2_idx) };
            block2 &= BIT_WRITE_MASKS[0][n];
            block2 |= data;
            *unsafe { self.data.get_unchecked_mut(block2_idx) } = block2;
        }

        self.cur_bit += n;

        Ok(())
    }

    // void bf_write::WriteByte( unsigned int val )
    pub fn write_u8(&mut self, data: u8) -> Result<()> {
        self.write_ubit64(data as u64, 8)
    }

    // NOTE: ref impl for varints:
    // https://github.com/rust-lang/rust/blob/e5b3e68abf170556b9d56c6f9028318e53c9f06b/compiler/rustc_serialize/src/leb128.rs

    // TODO: this can be faster
    //
    // void bf_write::WriteVarInt64( uint64 data )
    pub fn write_uvarint64(&mut self, data: u64) -> Result<()> {
        let mut data = data;
        loop {
            if data < 0x80 {
                self.write_u8(data as u8)?;
                break;
            }
            self.write_u8(((data & 0x7f) | 0x80) as u8)?;
            data >>= 7;
        }
        Ok(())
    }

    // TODO: this can be faster
    //
    // void bf_write::WriteVarInt32( uint32 data )
    pub fn write_uvarint32(&mut self, data: u32) -> Result<()> {
        let mut data = data;
        loop {
            if data < 0x80 {
                self.write_u8(data as u8)?;
                break;
            }
            self.write_u8(((data & 0x7f) | 0x80) as u8)?;
            data >>= 7;
        }
        Ok(())
    }

    // void bf_write::WriteSignedVarInt64( int64 data )
    pub fn write_varint64(&mut self, data: i64) -> Result<()> {
        self.write_uvarint64(zigzag_encode64(data))
    }

    // void bf_write::WriteSignedVarInt32( int32 data )
    pub fn write_varint32(&mut self, data: i32) -> Result<()> {
        self.write_uvarint32(zigzag_encode32(data))
    }
}
