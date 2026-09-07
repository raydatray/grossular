use crate::address::Address;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub(super) struct RecordInfo(u64);

impl RecordInfo {
    const PREVIOUS_ADDRESS_MASK: u64 = (1_u64 << 48) - 1;
    const TOMBSTONE_BIT: u64 = 1_u64 << 48;
    const VALID_BIT: u64 = 1_u64 << 49;
    const RESERVED_MASK: u64 = u64::MAX << 53;

    pub(super) const fn new_valid(previous: Address, tombstone: bool) -> Self {
        let mut value = previous.as_raw() | Self::VALID_BIT;

        if tombstone {
            value |= Self::TOMBSTONE_BIT;
        }

        Self(value)
    }

    pub(super) const fn from_raw(raw: u64) -> Self {
        assert!(
            raw & Self::RESERVED_MASK == 0,
            "record info contains reserved bits"
        );

        let previous_raw = raw & Self::PREVIOUS_ADDRESS_MASK;

        // validate the embedded address
        Address::from_raw(previous_raw);

        Self(raw)
    }

    pub(super) const fn as_raw(self) -> u64 {
        self.0
    }

    pub(super) const fn previous(self) -> Address {
        Address::from_raw(self.0 & Self::PREVIOUS_ADDRESS_MASK)
    }

    pub(super) const fn is_tombstone(self) -> bool {
        self.0 & Self::TOMBSTONE_BIT != 0
    }

    pub(super) const fn is_valid(self) -> bool {
        self.0 & Self::VALID_BIT != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn record_info_is_one_word() {
        assert_eq!(size_of::<RecordInfo>(), size_of::<u64>());
        assert_eq!(align_of::<RecordInfo>(), align_of::<u64>());
    }

    #[test]
    fn default_record_is_invalid_with_no_previous_address() {
        let info = RecordInfo::default();

        assert!(!info.is_valid());
        assert!(!info.is_tombstone());
        assert_eq!(info.previous(), Address::INVALID);
    }

    #[test]
    fn valid_record_round_trips_previous_address() {
        let previous = Address::from_offset(128);
        let info = RecordInfo::new_valid(previous, false);

        assert!(info.is_valid());
        assert!(!info.is_tombstone());
        assert_eq!(info.previous(), previous);
        assert_eq!(info.as_raw(), previous.as_raw() | RecordInfo::VALID_BIT);
    }

    #[test]
    fn tombstone_round_trips_without_changing_previous_address() {
        let previous = Address::from_offset(128);
        let info = RecordInfo::new_valid(previous, true);

        assert!(info.is_valid());
        assert!(info.is_tombstone());
        assert_eq!(info.previous(), previous);
        assert_eq!(
            info.as_raw(),
            previous.as_raw() | RecordInfo::VALID_BIT | RecordInfo::TOMBSTONE_BIT
        );
    }

    #[test]
    fn state_bits_do_not_overlap_previous_address() {
        let previous = Address::from_offset(256);
        let raw =
            previous.as_raw() | RecordInfo::VALID_BIT | RecordInfo::TOMBSTONE_BIT | (0b111 << 50);

        let info = RecordInfo::from_raw(raw);

        assert_eq!(info.previous(), previous);
        assert_eq!(info.as_raw(), raw);
    }

    #[test]
    #[should_panic(expected = "record info contains reserved bits")]
    fn rejects_reserved_bits() {
        RecordInfo::from_raw(RecordInfo::RESERVED_MASK);
    }

    #[test]
    #[should_panic]
    fn rejects_reserved_previous_addresses() {
        RecordInfo::from_raw(1 | RecordInfo::VALID_BIT);
    }
}
