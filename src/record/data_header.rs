use super::{ALIGNMENT, HEADER_SIZE};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub(super) struct RecordDataHeader(u64);

impl RecordDataHeader {
    const KEY_INLINE_BIT: u64 = 1 << 0;
    const VALUE_INLINE_BIT: u64 = 1 << 1;

    const FILLER_WORDS_SHIFT: u32 = 6;
    const KEY_LENGTH_SHIFT: u32 = 14;
    const VALUE_LENGTH_SHIFT: u32 = 24;

    const FILLER_WORDS_MASK: u64 = ((1_u64 << 8) - 1) << Self::FILLER_WORDS_SHIFT;
    const KEY_LENGTH_MASK: u64 = ((1_u64 << 10) - 1) << Self::KEY_LENGTH_SHIFT;
    const VALUE_LENGTH_MASK: u64 = ((1_u64 << 24) - 1) << Self::VALUE_LENGTH_SHIFT;

    const MAX_KEY_LENGTH: usize = (1 << 10) - 2;
    const MAX_VALUE_LENGTH: usize = (1 << 24) - 2;

    pub(super) const fn new_inline(key_len: usize, value_len: usize) -> Self {
        assert!(
            key_len <= Self::MAX_KEY_LENGTH,
            "key length exceeds header field"
        );
        assert!(
            value_len <= Self::MAX_VALUE_LENGTH,
            "value length exceeds header field"
        );

        Self(
            Self::KEY_INLINE_BIT
                | Self::VALUE_INLINE_BIT
                | ((key_len as u64) << Self::KEY_LENGTH_SHIFT)
                | ((value_len as u64) << Self::VALUE_LENGTH_SHIFT),
        )
    }

    pub(super) const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub(super) const fn as_raw(self) -> u64 {
        self.0
    }

    pub(super) const fn key_is_inline(self) -> bool {
        self.0 & Self::KEY_INLINE_BIT != 0
    }

    pub(super) const fn value_is_inline(self) -> bool {
        self.0 & Self::VALUE_INLINE_BIT != 0
    }

    pub(super) const fn key_len(self) -> usize {
        ((self.0 & Self::KEY_LENGTH_MASK) >> Self::KEY_LENGTH_SHIFT) as usize
    }

    pub(super) const fn value_len(self) -> usize {
        ((self.0 & Self::VALUE_LENGTH_MASK) >> Self::VALUE_LENGTH_SHIFT) as usize
    }

    pub(super) const fn filler_words(self) -> usize {
        ((self.0 & Self::FILLER_WORDS_MASK) >> Self::FILLER_WORDS_SHIFT) as usize
    }

    pub(super) const fn allocated_len(self) -> usize {
        let unaligned = HEADER_SIZE + self.key_len() + self.value_len();

        let aligned = unaligned.next_multiple_of(ALIGNMENT);

        aligned + self.filler_words() * ALIGNMENT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn data_header_is_one_word() {
        assert_eq!(size_of::<RecordDataHeader>(), size_of::<u64>());
        assert_eq!(align_of::<RecordDataHeader>(), align_of::<u64>());
    }

    #[test]
    fn default_header_has_empty_fields() {
        let header = RecordDataHeader::default();

        assert!(!header.key_is_inline());
        assert!(!header.value_is_inline());
        assert_eq!(header.key_len(), 0);
        assert_eq!(header.value_len(), 0);
        assert_eq!(header.filler_words(), 0);
    }

    #[test]
    fn inline_header_round_trips_lengths_and_flags() {
        let header = RecordDataHeader::new_inline(123, 45_678);

        assert!(header.key_is_inline());
        assert!(header.value_is_inline());
        assert_eq!(header.key_len(), 123);
        assert_eq!(header.value_len(), 45_678);
        assert_eq!(header.filler_words(), 0);
    }

    #[test]
    fn inline_header_uses_expected_bit_positions() {
        let header = RecordDataHeader::new_inline(5, 9);
        let expected = RecordDataHeader::KEY_INLINE_BIT
            | RecordDataHeader::VALUE_INLINE_BIT
            | (5 << RecordDataHeader::KEY_LENGTH_SHIFT)
            | (9 << RecordDataHeader::VALUE_LENGTH_SHIFT);

        assert_eq!(header.as_raw(), expected);
        assert_eq!(RecordDataHeader::from_raw(expected), header);
    }

    #[test]
    fn maximum_inline_lengths_round_trip() {
        let header = RecordDataHeader::new_inline(
            RecordDataHeader::MAX_KEY_LENGTH,
            RecordDataHeader::MAX_VALUE_LENGTH,
        );

        assert_eq!(header.key_len(), RecordDataHeader::MAX_KEY_LENGTH);
        assert_eq!(header.value_len(), RecordDataHeader::MAX_VALUE_LENGTH);
    }

    #[test]
    #[should_panic(expected = "key length exceeds header field")]
    fn rejects_oversized_inline_key() {
        RecordDataHeader::new_inline(RecordDataHeader::MAX_KEY_LENGTH + 1, 0);
    }

    #[test]
    #[should_panic(expected = "value length exceeds header field")]
    fn rejects_oversized_inline_value() {
        RecordDataHeader::new_inline(0, RecordDataHeader::MAX_VALUE_LENGTH + 1);
    }

    #[test]
    fn allocated_length_is_aligned() {
        for key_len in 0..16 {
            for value_len in 0..16 {
                let header = RecordDataHeader::new_inline(key_len, value_len);
                let allocated_len = header.allocated_len();

                assert!(allocated_len >= HEADER_SIZE + key_len + value_len);
                assert_eq!(allocated_len % ALIGNMENT, 0);
            }
        }
    }

    #[test]
    fn filler_words_extend_allocated_length() {
        let inline = RecordDataHeader::new_inline(1, 2);
        let with_filler = RecordDataHeader::from_raw(
            inline.as_raw() | (3 << RecordDataHeader::FILLER_WORDS_SHIFT),
        );

        assert_eq!(inline.allocated_len(), 24);
        assert_eq!(with_filler.filler_words(), 3);
        assert_eq!(with_filler.allocated_len(), 48);
    }
}
