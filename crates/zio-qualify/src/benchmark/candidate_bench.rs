//! Candidate construction and dispatch for one benchmark iteration.

use crate::Implementation;

use super::{
    measure::{Captured, Metric},
    mio_backend::MioBackend,
    polling_backend::PollingBackend,
    polling_direct,
    resource_limit::{self, Unsupported},
    scenario::Scenario,
    workload,
    zio_backend::ZioBackend,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Support {
    Available,
    Unavailable(Unsupported),
}

pub(crate) fn support(
    implementation: Implementation,
    scenario: Scenario,
) -> Result<Support, String> {
    if !scenario.supports(implementation) {
        return Ok(Support::Unavailable(Unsupported::capability(
            "the candidate does not expose this delivery profile",
        )));
    }
    if let Some(reason) = resource_limit::preflight(implementation, scenario)? {
        return Ok(Support::Unavailable(reason));
    }
    if implementation == Implementation::Polling && scenario == Scenario::LevelRepeat {
        return PollingBackend::supports_level().map(|supported| {
            if supported {
                Support::Available
            } else {
                Support::Unavailable(Unsupported::capability(
                    "the polling host backend reports no native Level support",
                ))
            }
        });
    }
    Ok(Support::Available)
}

pub(crate) fn run(
    implementation: Implementation,
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    match implementation {
        Implementation::Zio => workload::run::<ZioBackend>(scenario, iterations, metric),
        Implementation::Mio => workload::run::<MioBackend>(scenario, iterations, metric),
        Implementation::Polling => match scenario {
            Scenario::RegisterDelete => {
                polling_direct::register_delete(scenario, iterations, metric)
            }
            Scenario::ReadySingle | Scenario::ReadyBatch64 | Scenario::ReadyBatch1024 => {
                polling_direct::ready(scenario, iterations, metric)
            }
            _ => workload::run::<PollingBackend>(scenario, iterations, metric),
        },
    }
}

pub(crate) const fn version(implementation: Implementation) -> &'static str {
    match implementation {
        Implementation::Zio => env!("CARGO_PKG_VERSION"),
        Implementation::Mio => "1.2.2",
        Implementation::Polling => "3.11.0",
    }
}

pub(crate) const fn disclosure(implementation: Implementation) -> &'static str {
    match implementation {
        Implementation::Zio => {
            "Zio retains fixed event and registration capacities and duplicates every registered descriptor; those costs are included."
        }
        Implementation::Mio => {
            "Mio is measured under its native default; the API does not expose Level or OneShot selection."
        }
        Implementation::Polling => {
            "polling uses its native one-shot default for initial observations; the RAII adapter enforces delete-before-source-drop."
        }
    }
}
