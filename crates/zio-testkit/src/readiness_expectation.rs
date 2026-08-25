//! Required and allowed readiness masks for native conformance scenarios.

use zio::{Interest, Readiness};

use crate::{ReadinessFixture, ReadinessScenario};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExpectedReadiness {
    required: Readiness,
    required_any: Readiness,
    pub(crate) allowed: Readiness,
}

impl ExpectedReadiness {
    pub(crate) fn has_required(self, readiness: Readiness) -> bool {
        readiness.contains(self.required)
            && (self.required_any.is_empty() || intersects(readiness, self.required_any))
    }

    pub(crate) fn required_description(self) -> String {
        if self.required_any.is_empty() {
            format!("{:?}", self.required)
        } else {
            format!("{:?} plus one of {:?}", self.required, self.required_any)
        }
    }
}

pub(crate) fn expected_for(scenario: ReadinessScenario) -> ExpectedReadiness {
    let readable_closed = Readiness::READABLE.union(Readiness::READ_CLOSED);
    match scenario.fixture() {
        ReadinessFixture::UnixPendingEof | ReadinessFixture::TcpPendingEof => {
            let allowed = if scenario.interest() == Interest::READABLE.union(Interest::WRITABLE) {
                readable_closed.union(Readiness::WRITABLE)
            } else {
                readable_closed
            };
            exact_minimum(Readiness::READABLE, allowed)
        }
        ReadinessFixture::UnixWritable => exact_minimum(Readiness::WRITABLE, Readiness::WRITABLE),
        ReadinessFixture::PipePendingEof => exact_minimum(
            Readiness::READABLE,
            readable_closed.union(Readiness::WRITE_CLOSED),
        ),
        ReadinessFixture::PipeReaderClosed => ExpectedReadiness {
            required: Readiness::EMPTY,
            required_any: Readiness::WRITE_CLOSED.union(Readiness::ERROR),
            allowed: Readiness::WRITE_CLOSED
                .union(Readiness::WRITABLE)
                .union(Readiness::ERROR),
        },
    }
}

pub(crate) fn closure_for(scenario: ReadinessScenario) -> ExpectedReadiness {
    exact_minimum(Readiness::READ_CLOSED, expected_for(scenario).allowed)
}

fn exact_minimum(required: Readiness, allowed: Readiness) -> ExpectedReadiness {
    ExpectedReadiness {
        required,
        required_any: Readiness::EMPTY,
        allowed,
    }
}

fn intersects(left: Readiness, right: Readiness) -> bool {
    [
        Readiness::READABLE,
        Readiness::WRITABLE,
        Readiness::READ_CLOSED,
        Readiness::WRITE_CLOSED,
        Readiness::ERROR,
    ]
    .iter()
    .any(|flag| left.contains(*flag) && right.contains(*flag))
}
