use crate::{
    address::Address,
    record::{self, RecordRef},
};

pub(crate) struct Log {
    buffer: Vec<u8>,
}

impl Default for Log {
    fn default() -> Self {
        Self::new()
    }
}

impl Log {
    pub(crate) fn new() -> Self {
        Self {
            buffer: vec![0; Address::FIRST_VALID.as_offset()],
        }
    }

    pub(crate) fn tail(&self) -> Address {
        Address::from_offset(self.buffer.len())
    }

    pub(crate) fn append(&mut self, previous: Address, key: &[u8], value: &[u8]) -> Address {
        let address = self.tail();

        assert!(
            previous == Address::INVALID || previous.as_offset() < address.as_offset(),
            "previous address must refer to an older record"
        );

        let length = record::encoded_len(key.len(), value.len());
        let start = self.buffer.len();
        let end = start + length;

        // validate that the resulting tail is representable
        Address::from_offset(end);

        self.buffer.resize(end, 0);

        record::encode(&mut self.buffer[start..end], previous, key, value);

        address
    }

    pub(crate) fn read(&self, address: Address) -> RecordRef<'_> {
        assert_ne!(address, Address::INVALID, "cannot read an invalid address");

        let offset = address.as_offset();

        assert!(
            offset < self.buffer.len(),
            "address extends beyond the log tail"
        );

        record::decode(&self.buffer, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_log_starts_at_first_valid_address() {
        let log = Log::new();

        assert_eq!(log.tail(), Address::FIRST_VALID);
    }

    #[test]
    fn appends_and_reads_first_record() {
        let mut log = Log::new();

        let address = log.append(Address::INVALID, b"foo", b"one");
        let record = log.read(address);

        assert_eq!(address, Address::FIRST_VALID);
        assert_eq!(record.previous, Address::INVALID);
        assert_eq!(record.key, b"foo");
        assert_eq!(record.value, b"one");
    }

    #[test]
    fn appended_records_have_increasing_aligned_addresses() {
        let mut log = Log::new();

        let first = log.append(Address::INVALID, b"a", b"one");
        let second = log.append(Address::INVALID, b"bb", b"two");
        let third = log.append(Address::INVALID, b"ccc", b"three");

        assert!(first.as_offset() < second.as_offset());
        assert!(second.as_offset() < third.as_offset());
        assert_eq!(first.as_offset() % record::ALIGNMENT, 0);
        assert_eq!(second.as_offset() % record::ALIGNMENT, 0);
        assert_eq!(third.as_offset() % record::ALIGNMENT, 0);
        assert_eq!(log.tail().as_offset() % record::ALIGNMENT, 0);
    }

    #[test]
    fn records_form_a_backward_chain() {
        let mut log = Log::new();

        let first = log.append(Address::INVALID, b"foo", b"one");
        let second = log.append(first, b"foo", b"two");
        let third = log.append(second, b"foo", b"three");

        assert_eq!(log.read(third).previous, second);
        assert_eq!(log.read(second).previous, first);
        assert_eq!(log.read(first).previous, Address::INVALID);
    }

    #[test]
    #[should_panic(expected = "previous address must refer to an older record")]
    fn rejects_previous_address_at_or_after_tail() {
        let mut log = Log::new();
        let tail = log.tail();

        log.append(tail, b"foo", b"one");
    }

    #[test]
    #[should_panic(expected = "cannot read an invalid address")]
    fn rejects_invalid_address() {
        Log::new().read(Address::INVALID);
    }

    #[test]
    #[should_panic(expected = "address extends beyond the log tail")]
    fn rejects_address_at_tail() {
        let log = Log::new();

        log.read(log.tail());
    }
}
