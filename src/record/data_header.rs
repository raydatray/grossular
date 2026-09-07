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
