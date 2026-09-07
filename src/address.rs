//! Absolute address range and sentinels follow Tsavorite's `LogAddress`.
//! Grossular does not yet represent the read-cache marker in bit 47.
//!
//! Reference: `Tsavorite/cs/src/core/Index/Common/LogAddress.cs`
//! at Garnet commit `c9607605baa4`.

use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct Address(u64);

impl Address {
    const MAX_LOG_ADDRESS: u64 = (1_u64 << 47) - 1;

    pub const INVALID: Self = Self(0);
    pub const FIRST_VALID: Self = Self(64);

    pub(crate) const fn from_raw(raw: u64) -> Self {
        assert!(
            raw == Self::INVALID.0 || (raw >= Self::FIRST_VALID.0 && raw <= Self::MAX_LOG_ADDRESS)
        );

        Self(raw)
    }

    pub(crate) const fn as_raw(self) -> u64 {
        self.0
    }

    pub(crate) fn from_offset(offset: usize) -> Self {
        let raw = u64::try_from(offset).expect("log offset does not fit in u64");

        Self::from_raw(raw)
    }

    pub(crate) fn as_offset(self) -> usize {
        usize::try_from(self.0).expect("address does not fit in usize")
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::INVALID {
            f.write_str("invalid")
        } else {
            write!(f, "log:{}", self.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_address_is_invalid() {
        assert_eq!(Address::default(), Address::INVALID);
    }

    #[test]
    fn offset_round_trips() {
        let address = Address::from_offset(128);

        assert_eq!(address.as_offset(), 128);
        assert_eq!(address.as_raw(), 128);
    }

    #[test]
    fn displays_invalid_and_log_addresses() {
        assert_eq!(Address::INVALID.to_string(), "invalid");
        assert_eq!(Address::FIRST_VALID.to_string(), "log:64");
    }

    #[test]
    #[should_panic]
    fn rejects_reserved_addresses() {
        Address::from_raw(Address::FIRST_VALID.as_raw() - 1);
    }

    #[test]
    #[should_panic]
    fn rejects_addresses_above_the_log_range() {
        Address::from_raw(Address::MAX_LOG_ADDRESS + 1);
    }
}
