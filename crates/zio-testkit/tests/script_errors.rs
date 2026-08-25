//! Consumer-facing scripted backend diagnostics.

use std::os::unix::net::UnixStream;

use zio::{Error, Interest, Key, Mode, Operation};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptError, ScriptedPoll};

const KEY: Key = Key::new(404);

#[test]
fn script_reports_exhausted_operations() -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = ScriptedPoll::new(std::iter::empty::<MutationStep>())?;
    let source = UnixStream::pair()?.0;
    let result = poll.register(&source, KEY, Interest::READABLE, Mode::Level);
    assert!(result.is_err());
    assert_eq!(
        poll.finish(),
        Err(ScriptError::Exhausted {
            operation: Operation::Register,
        })
    );
    Ok(())
}

#[test]
fn script_reports_operation_mismatches() -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = ScriptedPoll::new([MutationStep::Delete(MutationOutcome::Success)])?;
    let source = UnixStream::pair()?.0;
    let result = poll.register(&source, KEY, Interest::READABLE, Mode::Level);
    assert!(result.is_err());
    assert_eq!(
        poll.finish(),
        Err(ScriptError::Mismatch {
            expected: Operation::Delete,
            actual: Operation::Register,
        })
    );
    Ok(())
}

#[test]
fn script_reports_remaining_steps() -> Result<(), Box<dyn std::error::Error>> {
    let poll = ScriptedPoll::new([MutationStep::Register(MutationOutcome::Success)])?;
    assert_eq!(poll.finish(), Err(ScriptError::Remaining { count: 1 }));
    Ok(())
}

#[test]
fn scripted_poll_rejects_zero_capacity() {
    let result = ScriptedPoll::with_capacity(0, std::iter::empty::<MutationStep>());
    assert!(matches!(result, Err(Error::Capacity { limit: 0 })));
}
