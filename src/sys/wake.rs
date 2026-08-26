//! Cloneable target-selected native wake.

use std::io;

/// Cloneable target-selected native wake.
#[derive(Clone, Debug)]
pub(crate) struct Wake {
    #[cfg(target_os = "linux")]
    pub(super) linux: std::sync::Arc<super::linux_group::Wake>,
    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(super) kqueue: super::kqueue_group::Wake,
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd"
    )))]
    pub(super) unsupported: std::sync::Arc<super::unsupported::Wake>,
}

impl Wake {
    pub(crate) fn wake(&self) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.linux.wake()
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            self.kqueue.wake()
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            self.unsupported.wake()
        }
    }
}
