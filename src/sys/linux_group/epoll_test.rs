//! Linux raw-event initialization boundary regressions.

use super::epoll::EpollBatch;

#[test]
fn unstamped_storage_never_exposes_an_event() -> Result<(), Box<dyn std::error::Error>> {
    let batch =
        EpollBatch::new(1).ok_or_else(|| std::io::Error::other("batch construction failed"))?;

    assert!(batch.event(0, 1).is_none());
    Ok(())
}
