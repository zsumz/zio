//! Encoded registration identity proofs.

use core::num::NonZeroU32;

use crate::{Error, RegistrationId};

use super::{decode, encode};

#[test]
fn sealed_parts_match_checked_decode_at_boundaries() -> Result<(), Error> {
    for (index, generation) in [(0, NonZeroU32::MIN), (u32::MAX - 1, NonZeroU32::MAX)] {
        let encoded = encode(index, generation).ok_or(Error::Invariant)?;

        assert_eq!(encoded.parts(), (index, generation.get()));
        assert_eq!(
            decode(encoded.id())?,
            (
                usize::try_from(index).map_err(|_| Error::Invariant)?,
                generation,
            )
        );
    }
    assert!(encode(u32::MAX, NonZeroU32::MIN).is_none());
    Ok(())
}

#[test]
fn checked_decode_rejects_invalid_raw_halves() {
    for raw in [0, 1, u64::from(u32::MIN) | (u64::from(1_u32) << u32::BITS)] {
        let id = RegistrationId::new(raw);
        assert!(matches!(
            decode(id),
            Err(Error::Stale { registration }) if registration == id
        ));
    }
}

#[test]
fn encoded_identity_round_trips_across_deterministic_samples() -> Result<(), Error> {
    let mut state = 0x6a09_e667_f3bc_c909_u64;
    for _ in 0..16_384 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let low = u32::try_from(state & u64::from(u32::MAX)).map_err(|_| Error::Invariant)?;
        let high = u32::try_from(state >> u32::BITS).map_err(|_| Error::Invariant)?;
        let index = low % u32::MAX;
        let generation = NonZeroU32::new(high | 1).ok_or(Error::Invariant)?;
        let encoded = encode(index, generation).ok_or(Error::Invariant)?;

        assert_eq!(encoded.parts(), (index, generation.get()));
        assert_eq!(
            decode(encoded.id())?,
            (
                usize::try_from(index).map_err(|_| Error::Invariant)?,
                generation,
            )
        );
    }
    Ok(())
}
