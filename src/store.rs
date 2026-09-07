use crate::{address::Address, hash::hash, index::Index, log::Log};

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
        let address = self.find_address(hash, key)?;
        let record = self.log.read(address);

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

    fn find_address(&self, hash: u64, key: &[u8]) -> Option<Address> {
        let mut address = self.index.head(hash);

        while address != Address::INVALID {
            let record = self.log.read(address);

            if record.key == key {
                return Some(address);
            }

            address = record.previous();
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
