//! Candidate-independent contract policy checks.

use std::io;

use zio_qualify::{ExpectedObservation, Observation, ProfileSupport, Scenario};

#[test]
fn contract_requires_every_portable_minimum() -> Result<(), io::Error> {
    let contract = ExpectedObservation::new(
        Observation::READABLE,
        Observation::EMPTY,
        Observation::READABLE | Observation::READ_CLOSED,
    );
    check(
        contract.validate(Observation::READ_CLOSED).is_err(),
        "contract accepted an observation without its required minimum",
    )?;
    check(
        contract.validate(Observation::READABLE).is_ok(),
        "contract rejected its required minimum",
    )
}

#[test]
fn contract_requires_one_declared_alternative() -> Result<(), io::Error> {
    let terminal = Observation::READ_CLOSED | Observation::ERROR;
    let contract = ExpectedObservation::new(Observation::EMPTY, terminal, terminal);
    check(
        contract.validate(Observation::EMPTY).is_err(),
        "contract accepted no terminal alternative",
    )?;
    check(
        contract.validate(Observation::ERROR).is_ok(),
        "contract rejected one declared terminal alternative",
    )
}

#[test]
fn contract_rejects_undocumented_observations() -> Result<(), io::Error> {
    let contract = ExpectedObservation::new(
        Observation::READABLE,
        Observation::EMPTY,
        Observation::READABLE,
    );
    check(
        contract
            .validate(Observation::READABLE | Observation::ERROR)
            .is_err(),
        "contract accepted an undocumented error flag",
    )
}

#[test]
fn scenario_names_are_stable() -> Result<(), io::Error> {
    let expected = [
        "unix.readable.initial_observation",
        "unix.writable.initial_observation",
        "unix.readable.level",
        "unix.writable.level",
        "unix.readable.one_shot",
        "unix.writable.one_shot",
    ];
    check(
        Scenario::ALL.map(Scenario::name) == expected,
        "qualification scenario names changed",
    )
}

#[test]
fn capability_labels_and_reasons_are_stable() -> Result<(), io::Error> {
    let unavailable = ProfileSupport::HostUnavailable {
        reason: "host limitation",
    };
    check(
        ProfileSupport::Native.name() == "native" && ProfileSupport::Native.reason().is_none(),
        "native capability accessors changed",
    )?;
    check(
        unavailable.name() == "host_unavailable" && unavailable.reason() == Some("host limitation"),
        "unavailable capability accessors changed",
    )
}

fn check(condition: bool, message: &'static str) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}
