#[allow(dead_code)]
mod bucket;

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

#[cfg(test)]
mod tests {
    use super::*;

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
