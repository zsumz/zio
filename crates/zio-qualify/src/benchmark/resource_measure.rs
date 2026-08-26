//! Live and retained descriptor observations outside timed regions.

use std::{fs, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Resources {
    pub(crate) live_fds: Option<LiveFds>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LiveFds {
    pub(crate) fixture_baseline: usize,
    pub(crate) candidate_setup: usize,
    pub(crate) active: usize,
    pub(crate) post_cleanup: usize,
}

impl LiveFds {
    pub(crate) const fn from_options(
        fixture_baseline: Option<usize>,
        candidate_setup: Option<usize>,
        active: Option<usize>,
        post_cleanup: Option<usize>,
    ) -> Option<Self> {
        match (fixture_baseline, candidate_setup, active, post_cleanup) {
            (Some(fixture_baseline), Some(candidate_setup), Some(active), Some(post_cleanup)) => {
                Some(Self {
                    fixture_baseline,
                    candidate_setup,
                    active,
                    post_cleanup,
                })
            }
            _ => None,
        }
    }

    pub(crate) fn setup_delta(self) -> Option<i64> {
        fd_delta(Some(self.fixture_baseline), Some(self.candidate_setup))
    }

    pub(crate) fn active_delta(self) -> Option<i64> {
        fd_delta(Some(self.fixture_baseline), Some(self.active))
    }

    pub(crate) fn cleanup_delta(self) -> Option<i64> {
        fd_delta(Some(self.fixture_baseline), Some(self.post_cleanup))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FdProbe {
    Available(PathBuf),
    Unavailable(&'static str),
}

impl FdProbe {
    pub(crate) fn discover() -> Self {
        for path in ["/proc/self/fd", "/dev/fd"] {
            if count_entries(path).is_ok() {
                return Self::Available(PathBuf::from(path));
            }
        }
        Self::Unavailable("neither /proc/self/fd nor /dev/fd is readable")
    }

    pub(crate) fn count(&self) -> Option<usize> {
        match self {
            Self::Available(path) => count_entries(path).ok(),
            Self::Unavailable(_) => None,
        }
    }

    pub(crate) fn path(&self) -> Option<&str> {
        match self {
            Self::Available(path) => path.to_str(),
            Self::Unavailable(_) => None,
        }
    }

    pub(crate) const fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Available(_) => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }
}

pub(crate) fn fd_delta(before: Option<usize>, after: Option<usize>) -> Option<i64> {
    let before = i128::try_from(before?).ok()?;
    let after = i128::try_from(after?).ok()?;
    i64::try_from(after - before).ok()
}

fn count_entries(path: impl AsRef<std::path::Path>) -> Result<usize, std::io::Error> {
    let mut count = 0_usize;
    for entry in fs::read_dir(path)? {
        entry?;
        count = count.saturating_add(1);
    }
    Ok(count)
}
