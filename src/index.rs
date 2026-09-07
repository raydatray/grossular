use crate::address::Address;

pub(crate) struct Index {
    slots: Vec<Address>,
    mask: usize,
}

impl Index {
    pub(crate) fn new(bucket_count: usize) -> Self {
        assert!(
            bucket_count.is_power_of_two(),
            "bucket count must be nonzero power of two"
        );

        Self {
            slots: vec![Address::INVALID; bucket_count],
            mask: bucket_count - 1,
        }
    }

    fn bucket(&self, hash: u64) -> usize {
        hash as usize & self.mask
    }

    pub(crate) fn head(&self, hash: u64) -> Address {
        self.slots[self.bucket(hash)]
    }

    pub(crate) fn set_head(&mut self, hash: u64, address: Address) {
        let bucket = self.bucket(hash);

        self.slots[bucket] = address;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
struct HashEntry(u64);

impl HashEntry {
    const ADDRESS_MASK: u64 = (1_u64 << 47) - 1;
    const TAG_SHIFT: u32 = 48;
    const TAG_MASK: u64 = (1_u64 << 15) - 1;

    const EMPTY: Self = Self(0);

    fn new(tag: u16, address: Address) -> Self {
        assert!(address != Address::INVALID);
        assert!(u64::from(tag) <= Self::TAG_MASK);

        Self((u64::from(tag) << Self::TAG_SHIFT) | address.as_raw())
    }

    fn is_empty(self) -> bool {
        self == Self::EMPTY
    }

    fn tag(self) -> u16 {
        ((self.0 >> Self::TAG_SHIFT) & Self::TAG_MASK) as u16
    }

    fn address(self) -> Address {
        Address::from_raw(self.0 & Self::ADDRESS_MASK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_entry_is_one_word() {
        assert_eq!(size_of::<HashEntry>(), size_of::<u64>());
    }

    #[test]
    fn empty_hash_entry_has_no_address() {
        assert!(HashEntry::EMPTY.is_empty());
        assert_eq!(HashEntry::EMPTY.address(), Address::INVALID);
    }

    #[test]
    fn hash_entry_round_trips_tag_and_address() {
        let address = Address::from_offset(128);
        let entry = HashEntry::new(0x1234, address);

        assert!(!entry.is_empty());
        assert_eq!(entry.tag(), 0x1234);
        assert_eq!(entry.address(), address);
    }

    #[test]
    fn hash_entry_accepts_the_largest_tag() {
        let address = Address::from_offset(128);
        let entry = HashEntry::new(HashEntry::TAG_MASK as u16, address);

        assert_eq!(entry.tag(), HashEntry::TAG_MASK as u16);
        assert_eq!(entry.address(), address);
    }

    #[test]
    #[should_panic]
    fn hash_entry_rejects_an_invalid_address() {
        HashEntry::new(1, Address::INVALID);
    }

    #[test]
    fn starts_with_invalid_heads() {
        let index = Index::new(8);

        for hash in 0..8 {
            assert_eq!(index.head(hash), Address::INVALID);
        }
    }

    #[test]
    fn sets_and_reads_head() {
        let mut index = Index::new(8);
        let address = Address::from_offset(64);

        index.set_head(3, address);

        assert_eq!(index.head(3), address);
    }

    #[test]
    fn hashes_with_same_low_bits_share_a_slot() {
        let mut index = Index::new(8);
        let first = Address::from_offset(64);
        let second = Address::from_offset(128);

        index.set_head(3, first);
        index.set_head(11, second); // 3 & 7 == 11 & 7

        assert_eq!(index.head(3), second);
        assert_eq!(index.head(11), second);
    }

    #[test]
    #[should_panic(expected = "bucket count must be nonzero power of two")]
    fn rejects_zero_buckets() {
        Index::new(0);
    }

    #[test]
    #[should_panic(expected = "bucket count must be nonzero power of two")]
    fn rejects_non_power_of_two() {
        Index::new(3);
    }
}
