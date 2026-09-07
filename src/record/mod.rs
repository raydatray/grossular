mod data_header;
mod info;

use crate::address::Address;
use data_header::RecordDataHeader;
use info::RecordInfo;

pub const HEADER_SIZE: usize = 16;
pub const ALIGNMENT: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecordKind {
    Value,
    Tombstone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordRef<'a> {
    info: RecordInfo,
    data_header: RecordDataHeader,
    pub(crate) key: &'a [u8],
    pub(crate) value: &'a [u8],
}

impl RecordRef<'_> {
    pub(crate) const fn previous(self) -> Address {
        self.info.previous()
    }

    pub(crate) const fn is_tombstone(self) -> bool {
        self.info.is_tombstone()
    }
}

pub(crate) const fn encoded_len(key_len: usize, value_len: usize) -> usize {
    RecordDataHeader::new_inline(key_len, value_len).allocated_len()
}

pub(crate) fn encode(
    destination: &mut [u8],
    previous: Address,
    kind: RecordKind,
    key: &[u8],
    value: &[u8],
) {
    let tombstone = matches!(kind, RecordKind::Tombstone);

    if tombstone {
        assert!(value.is_empty(), "tombstone records cannot contain a value");
    }

    let info = RecordInfo::new_valid(previous, tombstone);
    let data_header = RecordDataHeader::new_inline(key.len(), value.len());

    assert_eq!(
        destination.len(),
        data_header.allocated_len(),
        "record destination has wrong length"
    );

    let (header, payload) = destination.split_at_mut(HEADER_SIZE);

    header[0..8].copy_from_slice(&info.as_raw().to_le_bytes());
    header[8..16].copy_from_slice(&data_header.as_raw().to_le_bytes());

    let (key_destination, remaining) = payload.split_at_mut(key.len());

    let (value_destination, padding) = remaining.split_at_mut(value.len());

    key_destination.copy_from_slice(key);
    value_destination.copy_from_slice(value);
    padding.fill(0);
}

pub(crate) fn decode(buffer: &[u8], offset: usize) -> RecordRef<'_> {
    assert_eq!(offset % ALIGNMENT, 0, "record address is not aligned");

    let record = buffer
        .get(offset..)
        .expect("record address extends past the log");

    let header = record
        .get(..HEADER_SIZE)
        .expect("record header extends past the log");

    let info = RecordInfo::from_raw(u64::from_le_bytes(header[0..8].try_into().unwrap()));

    let data_header =
        RecordDataHeader::from_raw(u64::from_le_bytes(header[8..16].try_into().unwrap()));

    assert!(info.is_valid(), "record is invalid");
    assert!(data_header.key_is_inline(), "overflow keys not supported");
    assert!(
        data_header.value_is_inline(),
        "overflow values not supported"
    );

    let record_len = data_header.allocated_len();

    assert!(record_len <= record.len(), "record extends past the log");

    let payload = &record[HEADER_SIZE..record_len];
    let (key, remaining) = payload.split_at(data_header.key_len());
    let (value, _) = remaining.split_at(data_header.value_len());

    RecordRef {
        info,
        data_header,
        key,
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_lengths_are_aligned() {
        for key_len in 0..32 {
            for value_len in 0..32 {
                let len = encoded_len(key_len, value_len);

                assert!(len >= HEADER_SIZE + key_len + value_len);
                assert_eq!(len % ALIGNMENT, 0);
            }
        }
    }

    #[test]
    fn empty_record_uses_only_the_header() {
        assert_eq!(encoded_len(0, 0), HEADER_SIZE);
    }

    #[test]
    fn record_length_includes_padding() {
        assert_eq!(encoded_len(1, 0), 24);
        assert_eq!(encoded_len(3, 5), 24);
        assert_eq!(encoded_len(4, 5), 32);
    }

    #[test]
    fn encode_and_decode_round_trip() {
        let previous = Address::from_offset(128);
        let key = b"hello";
        let value = b"world";

        let mut buffer = vec![0; Address::FIRST_VALID.as_offset()];
        let offset = buffer.len();
        let length = encoded_len(key.len(), value.len());

        buffer.resize(offset + length, 0);

        encode(
            &mut buffer[offset..offset + length],
            previous,
            RecordKind::Value,
            key,
            value,
        );

        let record = decode(&buffer, offset);

        assert_eq!(record.previous(), previous);
        assert!(!record.is_tombstone());
        assert_eq!(record.key, key);
        assert_eq!(record.value, value);
    }

    #[test]
    fn tombstone_round_trips() {
        let previous = Address::from_offset(128);
        let key = b"hello";
        let length = encoded_len(key.len(), 0);
        let offset = Address::FIRST_VALID.as_offset();
        let mut buffer = vec![0; offset + length];

        encode(
            &mut buffer[offset..offset + length],
            previous,
            RecordKind::Tombstone,
            key,
            &[],
        );

        let record = decode(&buffer, offset);

        assert_eq!(record.previous(), previous);
        assert!(record.is_tombstone());
        assert_eq!(record.key, key);
        assert!(record.value.is_empty());
    }

    #[test]
    fn empty_value_is_not_a_tombstone() {
        let key = b"hello";
        let length = encoded_len(key.len(), 0);
        let offset = Address::FIRST_VALID.as_offset();
        let mut buffer = vec![0; offset + length];

        encode(
            &mut buffer[offset..offset + length],
            Address::INVALID,
            RecordKind::Value,
            key,
            &[],
        );

        let record = decode(&buffer, offset);

        assert!(!record.is_tombstone());
        assert!(record.value.is_empty());
    }

    #[test]
    fn padding_is_zeroed() {
        let key = b"a";
        let value = b"bc";
        let length = encoded_len(key.len(), value.len());

        // fill with a nonzero value so the test proves encode clears padding
        let mut destination = vec![0xff; length];

        encode(
            &mut destination,
            Address::INVALID,
            RecordKind::Value,
            key,
            value,
        );

        let data_end = HEADER_SIZE + key.len() + value.len();

        assert!(destination[data_end..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn every_alignment_remainder_round_trips() {
        for key_len in 0..16 {
            for value_len in 0..16 {
                let key = vec![b'k'; key_len];
                let value = vec![b'v'; value_len];
                let length = encoded_len(key_len, value_len);

                let offset = Address::FIRST_VALID.as_offset();
                let mut buffer = vec![0; offset + length];

                encode(
                    &mut buffer[offset..offset + length],
                    Address::INVALID,
                    RecordKind::Value,
                    &key,
                    &value,
                );

                let record = decode(&buffer, offset);

                assert_eq!(record.previous(), Address::INVALID);
                assert!(!record.is_tombstone());
                assert_eq!(record.key, key);
                assert_eq!(record.value, value);
            }
        }
    }

    #[test]
    #[should_panic(expected = "tombstone records cannot contain a value")]
    fn rejects_tombstone_with_value() {
        let key = b"hello";
        let value = b"value";
        let length = encoded_len(key.len(), value.len());
        let mut destination = vec![0; length];

        encode(
            &mut destination,
            Address::INVALID,
            RecordKind::Tombstone,
            key,
            value,
        );
    }

    #[test]
    #[should_panic(expected = "record address is not aligned")]
    fn rejects_unaligned_offset() {
        decode(&[0; 64], 1);
    }

    #[test]
    #[should_panic(expected = "record header extends past the log")]
    fn rejects_truncated_header() {
        decode(&[0; HEADER_SIZE - 1], 0);
    }

    #[test]
    #[should_panic(expected = "record extends past the log")]
    fn rejects_truncated_body() {
        let mut buffer = vec![0; HEADER_SIZE];
        let info = RecordInfo::new_valid(Address::INVALID, false);
        let data_header = RecordDataHeader::new_inline(10, 20);

        buffer[0..8].copy_from_slice(&info.as_raw().to_le_bytes());
        buffer[8..16].copy_from_slice(&data_header.as_raw().to_le_bytes());

        decode(&buffer, 0);
    }
}
