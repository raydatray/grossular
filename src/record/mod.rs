use crate::address::Address;

pub const HEADER_SIZE: usize = 16;
pub const ALIGNMENT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordRef<'a> {
    pub previous: Address,
    pub key: &'a [u8],
    pub value: &'a [u8],
}

pub(crate) fn encoded_len(key_len: usize, value_len: usize) -> usize {
    assert!(
        key_len <= u32::MAX as usize,
        "key length exceeds the record format"
    );
    assert!(
        value_len <= u32::MAX as usize,
        "value length exceeds the record format"
    );

    (HEADER_SIZE + key_len + value_len).next_multiple_of(ALIGNMENT)
}

pub(crate) fn encode(destination: &mut [u8], previous: Address, key: &[u8], value: &[u8]) {
    let required_len = encoded_len(key.len(), value.len());

    assert_eq!(
        destination.len(),
        required_len,
        "record destination has the wrong length"
    );

    let key_len = u32::try_from(key.len()).expect("key length exceeds the record format");
    let value_len = u32::try_from(value.len()).expect("value length exceeds the record format");

    let (header, payload) = destination.split_at_mut(HEADER_SIZE);

    header[0..8].copy_from_slice(&previous.as_raw().to_le_bytes());
    header[8..12].copy_from_slice(&key_len.to_le_bytes());
    header[12..16].copy_from_slice(&value_len.to_le_bytes());

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

    let previous = Address::from_raw(u64::from_le_bytes(
        header[0..8]
            .try_into()
            .expect("previous address has the wrong length"),
    ));

    let key_len = u32::from_le_bytes(
        header[8..12]
            .try_into()
            .expect("key length has the wrong length"),
    ) as usize;

    let value_len = u32::from_le_bytes(
        header[12..16]
            .try_into()
            .expect("value length has the wrong length"),
    ) as usize;

    let record_len = encoded_len(key_len, value_len);

    assert!(record_len <= record.len(), "record extends past the log");

    let payload = &record[HEADER_SIZE..record_len];
    let (key, remaining) = payload.split_at(key_len);
    let (value, _) = remaining.split_at(value_len);

    RecordRef {
        previous,
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

        encode(&mut buffer[offset..offset + length], previous, key, value);

        let record = decode(&buffer, offset);

        assert_eq!(record.previous, previous);
        assert_eq!(record.key, key);
        assert_eq!(record.value, value);
    }

    #[test]
    fn padding_is_zeroed() {
        let key = b"a";
        let value = b"bc";
        let length = encoded_len(key.len(), value.len());

        // fill with a nonzero value so the test proves encode clears padding
        let mut destination = vec![0xff; length];

        encode(&mut destination, Address::INVALID, key, value);

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
                    &key,
                    &value,
                );

                let record = decode(&buffer, offset);

                assert_eq!(record.previous, Address::INVALID);
                assert_eq!(record.key, key);
                assert_eq!(record.value, value);
            }
        }
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

        buffer[8..12].copy_from_slice(&10_u32.to_le_bytes());
        buffer[12..16].copy_from_slice(&20_u32.to_le_bytes());

        decode(&buffer, 0);
    }
}
