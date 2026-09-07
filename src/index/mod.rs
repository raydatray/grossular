mod bucket;

use crate::address::Address;
use bucket::{Bucket, BucketLookup, BucketUpdate};

pub(crate) struct Index {
    primary: Box<[Bucket]>,
    overflow: Vec<Bucket>,
    mask: usize,
}

#[derive(Clone, Copy)]
enum BucketLocation {
    Primary(usize),
    Overflow(usize),
}

impl Index {
    pub(crate) fn new(bucket_count: usize) -> Self {
        assert!(
            bucket_count.is_power_of_two(),
            "bucket count must be nonzero power of two"
        );

        Self {
            primary: vec![Bucket::default(); bucket_count].into_boxed_slice(),
            overflow: Vec::new(),
            mask: bucket_count - 1,
        }
    }

    fn bucket_index(&self, hash: u64) -> usize {
        hash as usize & self.mask
    }

    fn tag(hash: u64) -> u16 {
        (hash >> 49) as u16
    }

    fn bucket_at(&self, location: BucketLocation) -> &Bucket {
        match location {
            BucketLocation::Primary(index) => &self.primary[index],
            BucketLocation::Overflow(index) => &self.overflow[index],
        }
    }

    fn bucket_at_mut(&mut self, location: BucketLocation) -> &mut Bucket {
        match location {
            BucketLocation::Primary(index) => &mut self.primary[index],
            BucketLocation::Overflow(index) => &mut self.overflow[index],
        }
    }

    pub(crate) fn head(&self, hash: u64) -> Address {
        let tag = Self::tag(hash);
        let mut location = BucketLocation::Primary(self.bucket_index(hash));

        loop {
            match self.bucket_at(location).lookup(tag) {
                BucketLookup::Found(address) => return address,
                BucketLookup::FollowOverflow(index) => {
                    location = BucketLocation::Overflow(index);
                }
                BucketLookup::Missing => return Address::INVALID,
            }
        }
    }

    pub(crate) fn set_head(&mut self, hash: u64, address: Address) {
        let tag = Self::tag(hash);
        let mut location = BucketLocation::Primary(self.bucket_index(hash));

        loop {
            match self.bucket_at_mut(location).update(tag, address) {
                BucketUpdate::Complete => return,
                BucketUpdate::FollowOverflow(index) => {
                    location = BucketLocation::Overflow(index);
                }
                BucketUpdate::NeedsOverflow => {
                    let index = self.overflow.len();

                    self.overflow.push(Bucket::with_entry(tag, address));
                    self.bucket_at_mut(location).set_overflow(index);
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash(bucket: u64, tag: u16) -> u64 {
        (u64::from(tag) << 49) | bucket
    }

    fn address(index: usize) -> Address {
        Address::from_offset(64 + index * 64)
    }

    #[test]
    fn starts_with_invalid_heads() {
        let index = Index::new(8);

        for bucket in 0..8 {
            assert_eq!(index.head(test_hash(bucket, 1)), Address::INVALID);
        }
    }

    #[test]
    fn sets_and_reads_head() {
        let mut index = Index::new(8);
        let address = address(0);

        index.set_head(test_hash(3, 1), address);

        assert_eq!(index.head(test_hash(3, 1)), address);
    }

    #[test]
    fn different_tags_in_one_bucket_have_independent_heads() {
        let mut index = Index::new(8);
        let first_hash = test_hash(3, 1);
        let second_hash = test_hash(3, 2);

        index.set_head(first_hash, address(0));
        index.set_head(second_hash, address(1));

        assert_eq!(index.head(first_hash), address(0));
        assert_eq!(index.head(second_hash), address(1));
    }

    #[test]
    fn hashes_with_the_same_bucket_and_tag_share_a_head() {
        let mut index = Index::new(8);
        let first_hash = test_hash(3, 7);
        let second_hash = first_hash | (1 << 20);

        index.set_head(first_hash, address(0));
        index.set_head(second_hash, address(1));

        assert_eq!(index.head(first_hash), address(1));
        assert_eq!(index.head(second_hash), address(1));
    }

    #[test]
    fn seven_tags_fit_without_overflow() {
        let mut index = Index::new(1);

        for tag in 0..7 {
            index.set_head(test_hash(0, tag), address(tag as usize));
        }

        assert!(index.overflow.is_empty());

        for tag in 0..7 {
            assert_eq!(index.head(test_hash(0, tag)), address(tag as usize));
        }
    }

    #[test]
    fn eighth_tag_allocates_an_overflow_bucket() {
        let mut index = Index::new(1);

        for tag in 0..8 {
            index.set_head(test_hash(0, tag), address(tag as usize));
        }

        assert_eq!(index.overflow.len(), 1);

        for tag in 0..8 {
            assert_eq!(index.head(test_hash(0, tag)), address(tag as usize));
        }
    }

    #[test]
    fn fifteenth_tag_allocates_a_second_overflow_bucket() {
        let mut index = Index::new(1);

        for tag in 0..15 {
            index.set_head(test_hash(0, tag), address(tag as usize));
        }

        assert_eq!(index.overflow.len(), 2);

        for tag in 0..15 {
            assert_eq!(index.head(test_hash(0, tag)), address(tag as usize));
        }
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
