use crate::address::Address;

pub(super) const ENTRIES_PER_BUCKET: usize = 7;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub(super) struct HashEntry(u64);

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

const OVERFLOW_MASK: u64 = (1_u64 << 48) - 1;

#[derive(Clone, Default)]
#[repr(C, align(64))]
pub(super) struct Bucket {
    entries: [HashEntry; ENTRIES_PER_BUCKET],
    control: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum BucketLookup {
    Found(Address),
    FollowOverflow(usize),
    Missing,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum BucketUpdate {
    Complete,
    FollowOverflow(usize),
    NeedsOverflow,
}

impl Bucket {
    pub(super) fn with_entry(tag: u16, address: Address) -> Self {
        let mut bucket = Self::default();
        bucket.entries[0] = HashEntry::new(tag, address);

        bucket
    }

    fn overflow(&self) -> Option<usize> {
        let encoded = self.control & OVERFLOW_MASK;

        encoded.checked_sub(1).map(|index| index as usize)
    }

    pub(super) fn set_overflow(&mut self, index: usize) {
        let encoded = index.checked_add(1).expect("overflow index overflow");

        assert!(encoded as u64 <= OVERFLOW_MASK);

        self.control = (self.control & !OVERFLOW_MASK) | encoded as u64;
    }

    pub(super) fn update(&mut self, tag: u16, address: Address) -> BucketUpdate {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| !entry.is_empty() && entry.tag() == tag)
        {
            *entry = HashEntry::new(tag, address);
            return BucketUpdate::Complete;
        }

        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.is_empty()) {
            *entry = HashEntry::new(tag, address);
            return BucketUpdate::Complete;
        }

        match self.overflow() {
            Some(index) => BucketUpdate::FollowOverflow(index),
            None => BucketUpdate::NeedsOverflow,
        }
    }

    pub(super) fn lookup(&self, tag: u16) -> BucketLookup {
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| !entry.is_empty() && entry.tag() == tag)
        {
            return BucketLookup::Found(entry.address());
        }

        match self.overflow() {
            Some(index) => BucketLookup::FollowOverflow(index),
            None => BucketLookup::Missing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    fn address(index: usize) -> Address {
        Address::from_offset(64 + index * 64)
    }

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
        let entry = HashEntry::new(0x1234, address(0));

        assert!(!entry.is_empty());
        assert_eq!(entry.tag(), 0x1234);
        assert_eq!(entry.address(), address(0));
    }

    #[test]
    fn hash_entry_accepts_the_largest_tag() {
        let entry = HashEntry::new(HashEntry::TAG_MASK as u16, address(0));

        assert_eq!(entry.tag(), HashEntry::TAG_MASK as u16);
        assert_eq!(entry.address(), address(0));
    }

    #[test]
    #[should_panic]
    fn hash_entry_rejects_an_invalid_address() {
        HashEntry::new(1, Address::INVALID);
    }

    #[test]
    fn bucket_is_one_cache_line() {
        assert_eq!(size_of::<Bucket>(), 64);
        assert_eq!(align_of::<Bucket>(), 64);
    }

    #[test]
    fn bucket_stores_seven_distinct_tags() {
        let mut bucket = Bucket::default();

        for tag in 0..ENTRIES_PER_BUCKET as u16 {
            assert_eq!(
                bucket.update(tag, address(tag as usize)),
                BucketUpdate::Complete
            );
        }

        for tag in 0..ENTRIES_PER_BUCKET as u16 {
            assert_eq!(
                bucket.lookup(tag),
                BucketLookup::Found(address(tag as usize))
            );
        }

        assert_eq!(
            bucket.update(ENTRIES_PER_BUCKET as u16, address(ENTRIES_PER_BUCKET)),
            BucketUpdate::NeedsOverflow
        );
    }

    #[test]
    fn bucket_encodes_one_based_overflow_indexes() {
        let mut bucket = Bucket::default();

        for tag in 0..ENTRIES_PER_BUCKET as u16 {
            assert_eq!(
                bucket.update(tag, address(tag as usize)),
                BucketUpdate::Complete
            );
        }

        bucket.set_overflow(0);

        assert_eq!(bucket.lookup(99), BucketLookup::FollowOverflow(0));
        assert_eq!(
            bucket.update(99, address(ENTRIES_PER_BUCKET)),
            BucketUpdate::FollowOverflow(0)
        );
    }
}
