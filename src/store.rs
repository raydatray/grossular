use crate::{address::Address, hash::hash, index::Index, log::Log, record::RecordRef};

pub struct Store {
    index: Index,
    log: Log,
}

impl Store {
    pub fn new(bucket_count: usize) -> Self {
        Self {
            index: Index::new(bucket_count),
            log: Log::new(),
        }
    }

    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        let hash = hash(key);
        let (_, record) = self.find_record(hash, key)?;

        if record.is_tombstone() {
            None
        } else {
            Some(record.value)
        }
    }

    pub fn upsert(&mut self, key: &[u8], value: &[u8]) {
        let hash = hash(key);
        let previous = self.index.head(hash);
        let address = self.log.append(previous, key, value);

        self.index.set_head(hash, address);
    }

    pub fn delete(&mut self, key: &[u8]) -> bool {
        let hash = hash(key);

        let Some((_, record)) = self.find_record(hash, key) else {
            return false;
        };

        if record.is_tombstone() {
            return false;
        }

        let previous = self.index.head(hash);
        let tombstone = self.log.append_tombstone(previous, key);

        self.index.set_head(hash, tombstone);

        true
    }

    fn find_record(&self, hash: u64, key: &[u8]) -> Option<(Address, RecordRef<'_>)> {
        let mut address = self.index.head(hash);

        while address != Address::INVALID {
            let record = self.log.read(address);

            if record.key == key {
                return Some((address, record));
            }

            address = record.previous();
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same_tag_keys() -> ([u8; 8], [u8; 8]) {
        let first = 1_u64.to_le_bytes();
        let second = 7_313_u64.to_le_bytes();

        assert_eq!(hash(&first) >> 49, hash(&second) >> 49);

        (first, second)
    }

    #[test]
    fn missing_key_returns_none() {
        let store = Store::new(8);

        assert_eq!(store.get(b"missing"), None);
    }

    #[test]
    fn upserts_and_reads_value() {
        let mut store = Store::new(8);

        store.upsert(b"foo", b"one");

        assert_eq!(store.get(b"foo"), Some(b"one".as_slice()));
    }

    #[test]
    fn newest_value_shadows_older_value() {
        let mut store = Store::new(8);

        store.upsert(b"foo", b"one");
        store.upsert(b"foo", b"two");
        store.upsert(b"foo", b"three");

        assert_eq!(store.get(b"foo"), Some(b"three".as_slice()));
    }

    #[test]
    fn colliding_keys_share_a_chain() {
        // One bucket forces every key into the same chain.
        let mut store = Store::new(1);

        store.upsert(b"foo", b"one");
        store.upsert(b"bar", b"two");
        store.upsert(b"baz", b"three");

        assert_eq!(store.get(b"foo"), Some(b"one".as_slice()));
        assert_eq!(store.get(b"bar"), Some(b"two".as_slice()));
        assert_eq!(store.get(b"baz"), Some(b"three".as_slice()));
    }

    #[test]
    fn collision_does_not_produce_false_match() {
        let mut store = Store::new(1);

        store.upsert(b"foo", b"value");

        assert_eq!(store.get(b"bar"), None);
    }

    #[test]
    fn overwrite_preserves_other_colliding_keys() {
        let mut store = Store::new(1);

        store.upsert(b"foo", b"foo-one");
        store.upsert(b"bar", b"bar-one");
        store.upsert(b"foo", b"foo-two");

        assert_eq!(store.get(b"foo"), Some(b"foo-two".as_slice()));
        assert_eq!(store.get(b"bar"), Some(b"bar-one".as_slice()));
    }

    #[test]
    fn supports_empty_keys_and_values() {
        let mut store = Store::new(1);

        store.upsert(b"", b"");
        store.upsert(b"key", b"");

        assert_eq!(store.get(b""), Some(b"".as_slice()));
        assert_eq!(store.get(b"key"), Some(b"".as_slice()));
    }

    #[test]
    fn deletes_existing_key() {
        let mut store = Store::new(8);
        store.upsert(b"foo", b"value");

        assert!(store.delete(b"foo"));
        assert_eq!(store.get(b"foo"), None);
    }

    #[test]
    fn deleting_missing_key_returns_false() {
        let mut store = Store::new(8);

        assert!(!store.delete(b"missing"));
    }

    #[test]
    fn repeated_delete_returns_false() {
        let mut store = Store::new(8);
        store.upsert(b"foo", b"value");

        assert!(store.delete(b"foo"));
        assert!(!store.delete(b"foo"));
    }

    #[test]
    fn upsert_after_delete_restores_key() {
        let mut store = Store::new(8);
        store.upsert(b"foo", b"old");
        store.delete(b"foo");

        store.upsert(b"foo", b"new");

        assert_eq!(store.get(b"foo"), Some(b"new".as_slice()));
    }

    #[test]
    fn deleting_one_key_preserves_colliding_keys() {
        let mut store = Store::new(1);
        store.upsert(b"foo", b"foo-value");
        store.upsert(b"bar", b"bar-value");

        assert!(store.delete(b"foo"));
        assert_eq!(store.get(b"foo"), None);
        assert_eq!(store.get(b"bar"), Some(b"bar-value".as_slice()));
    }

    #[test]
    fn different_keys_with_the_same_tag_remain_readable() {
        let mut store = Store::new(1);
        let (first, second) = same_tag_keys();

        store.upsert(&first, b"first-value");
        store.upsert(&second, b"second-value");

        assert_eq!(store.get(&first), Some(b"first-value".as_slice()));
        assert_eq!(store.get(&second), Some(b"second-value".as_slice()));
    }

    #[test]
    fn updating_one_same_tag_key_preserves_the_other() {
        let mut store = Store::new(1);
        let (first, second) = same_tag_keys();

        store.upsert(&first, b"first-old");
        store.upsert(&second, b"second-value");
        store.upsert(&first, b"first-new");

        assert_eq!(store.get(&first), Some(b"first-new".as_slice()));
        assert_eq!(store.get(&second), Some(b"second-value".as_slice()));
    }

    #[test]
    fn deleting_one_same_tag_key_preserves_the_other() {
        let mut store = Store::new(1);
        let (first, second) = same_tag_keys();

        store.upsert(&first, b"first-value");
        store.upsert(&second, b"second-value");

        assert!(store.delete(&first));
        assert_eq!(store.get(&first), None);
        assert_eq!(store.get(&second), Some(b"second-value".as_slice()));
    }
}
