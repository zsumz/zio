//! File-descriptor preflight for large kqueue fixtures.

use super::{config::Row, model::Resources};
use crate::benchmark::{measure::FdProbe, resource_limit::soft_limit};

pub(super) fn inspect(row: Row) -> Result<Resources, String> {
    let probe = FdProbe::discover();
    let open_fds = probe.count().and_then(|value| u64::try_from(value).ok());
    let (soft_fd_limit, fd_limit_source) =
        soft_limit().map_or((None, None), |(limit, source)| (Some(limit), Some(source)));
    let required_additional_fds = u64::try_from(row.registrations)
        .map_err(display)?
        .checked_add(8)
        .ok_or_else(|| "descriptor requirement overflow".to_owned())?;
    Ok(Resources {
        open_fds,
        soft_fd_limit,
        fd_limit_source,
        required_additional_fds,
    })
}

pub(super) fn unsupported(resources: Resources) -> Option<(&'static str, String)> {
    if !cfg!(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd"
    )) {
        return Some((
            "kqueue_unavailable",
            "the host is not a supported kqueue target".to_owned(),
        ));
    }
    match (resources.open_fds, resources.soft_fd_limit) {
        (Some(open), Some(limit))
            if open.saturating_add(resources.required_additional_fds) > limit =>
        {
            Some((
                "insufficient_fd_limit",
                format!(
                    "row needs {} additional descriptors with {open} already open, above soft limit {limit}",
                    resources.required_additional_fds
                ),
            ))
        }
        _ => None,
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
