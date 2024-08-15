use std::io;

use dungers_varint::{
    read_uvarint32, read_uvarint64, read_varint32, read_varint64, write_uvarint32, write_uvarint64,
    write_varint32, write_varint64,
};

macro_rules! impl_test_uvarint {
    ($test_name:ident, $write_fn_name:ident, $read_fn_name:ident, $int_ty:ident) => {
        #[test]
        fn $test_name() {
            // Test 256 evenly spaced values of integer range,
            // integer max value, and some "random" numbers.
            let mut values = Vec::new();

            let increment = (1 as $int_ty) << ($int_ty::BITS - 8);
            values.extend((0..256).map(|i| $int_ty::MIN + i * increment));

            values.push($int_ty::MAX);

            values.extend(
                (-500..500).map(|i| (i as $int_ty).wrapping_mul(0x12345789ABCDEFu64 as $int_ty)),
            );

            let mut buf = [0u8; 1 << 20];
            let mut cursor = io::Cursor::new(&mut buf[..]);

            for x in &values {
                $write_fn_name(&mut cursor, *x).unwrap();
            }

            use io::Seek;
            cursor.seek(io::SeekFrom::Start(0)).unwrap();

            for want in &values {
                let (got, _) = $read_fn_name(&mut cursor).unwrap();
                assert_eq!(got, *want);
            }
        }
    };
}

impl_test_uvarint!(test_uvarint32, write_uvarint32, read_uvarint32, u32);
impl_test_uvarint!(test_uvarint64, write_uvarint64, read_uvarint64, u64);

macro_rules! impl_test_varint {
    ($test_name:ident, $write_fn_name:ident, $read_fn_name:ident, $int_ty:ident) => {
        #[test]
        fn $test_name() {
            // Test 256 evenly spaced values of integer range,
            // integer max value, and some "random" numbers.
            let mut values = Vec::new();

            let mut value = $int_ty::MIN;
            let increment = (1 as $int_ty) << ($int_ty::BITS - 8);

            for _ in 0..256 {
                values.push(value);
                // The addition in the last loop iteration overflows.
                value = value.wrapping_add(increment);
            }

            values.push($int_ty::MAX);

            values.extend(
                (-500..500).map(|i| (i as $int_ty).wrapping_mul(0x12345789ABCDEFi64 as $int_ty)),
            );

            let mut buf = [0u8; 1 << 20];
            let mut cursor = io::Cursor::new(&mut buf[..]);

            for x in &values {
                $write_fn_name(&mut cursor, *x).unwrap();
            }

            use io::Seek;
            cursor.seek(io::SeekFrom::Start(0)).unwrap();

            for want in &values {
                let (got, _) = $read_fn_name(&mut cursor).unwrap();
                assert_eq!(got, *want);
            }
        }
    };
}

impl_test_varint!(test_varint32, write_varint32, read_varint32, i32);
impl_test_varint!(test_varint64, write_varint64, read_varint64, i64);
