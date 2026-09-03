//! Poller builder regressions.

use crate::{
    CapacityKind, CapacityReason, DEFAULT_EVENT_CAPACITY, DEFAULT_REGISTRATION_CAPACITY, Error,
    Poll, PollBuilder,
};

#[test]
fn builder_uses_the_standard_defaults() -> Result<(), Error> {
    assert_eq!(Poll::builder(), PollBuilder::new());
    assert_eq!(Poll::builder(), PollBuilder::default());

    let poll = Poll::builder().build()?;
    assert_eq!(poll.event_capacity(), DEFAULT_EVENT_CAPACITY);
    assert_eq!(poll.registration_capacity(), DEFAULT_REGISTRATION_CAPACITY);
    Ok(())
}

#[test]
fn builder_names_custom_capacities() -> Result<(), Error> {
    let poll = Poll::builder()
        .event_capacity(3)
        .registration_capacity(5)
        .build()?;

    assert_eq!(poll.event_capacity(), 3);
    assert_eq!(poll.registration_capacity(), 5);
    Ok(())
}

#[test]
fn builder_defers_capacity_validation_to_build() {
    assert!(matches!(
        Poll::builder().event_capacity(0).build(),
        Err(Error::Capacity {
            kind: CapacityKind::Event,
            limit: 0,
            reason: CapacityReason::Zero,
        })
    ));
    assert!(matches!(
        Poll::builder().registration_capacity(0).build(),
        Err(Error::Capacity {
            kind: CapacityKind::Registration,
            limit: 0,
            reason: CapacityReason::Zero,
        })
    ));
}
