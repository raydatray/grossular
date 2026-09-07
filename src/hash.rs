//! Port of Tsavorite's `Utility.HashBytes`.
//!
//! Reference: `Tsavorite/cs/src/core/Utilities/Utility.cs`
//! at Garnet commit `c9607605baa4`.

const HASH_MULTIPLIER: u64 = 40_343;

pub(crate) fn hash(key: &[u8]) -> u64 {
    key.chunks(2)
        .fold(key.len() as u64, |state, chunk| {
            let word = match chunk {
                [low, high] => u16::from_le_bytes([*low, *high]) as u64,
                [last] => *last as u64,
                _ => unreachable!(),
            };

            state.wrapping_mul(HASH_MULTIPLIER).wrapping_add(word)
        })
        .wrapping_mul(HASH_MULTIPLIER)
        .rotate_right(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_tsavorite_hash_vectors() {
        let vectors: &[(&[u8], u64)] = &[
            (b"", 0x0000_0000_0000_0000),
            (b"a", 0x8000_0000_0613_e454),
            (b"ab", 0x9000_0000_0fe9_4a25),
            (b"abc", 0xb000_0d86_f1bb_918e),
            (b"hello", 0x2cf7_6a0c_27fc_b1d2),
            (b"tsavorite", 0xbee9_9b55_46d6_2a1b),
        ];

        for &(key, expected) in vectors {
            assert_eq!(hash(key), expected, "key: {key:?}");
        }
    }

    #[test]
    fn wraps_for_large_keys() {
        assert_eq!(hash(&vec![0xff; 100_000]), 0x063c_eec2_a12f_3042);
    }
}
