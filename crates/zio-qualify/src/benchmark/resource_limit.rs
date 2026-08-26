//! Descriptor-limit preflight and structured unsupported results.

use std::{fs, process::Command};

use crate::Implementation;

use super::{measure::FdProbe, scenario::Scenario};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Unsupported {
    pub(crate) code: &'static str,
    pub(crate) reason: String,
    pub(crate) required_additional_fds: Option<u64>,
    pub(crate) observed_open_fds: Option<u64>,
    pub(crate) observed_soft_fd_limit: Option<u64>,
    pub(crate) fd_limit_source: Option<&'static str>,
}

impl Unsupported {
    pub(crate) fn capability(reason: &'static str) -> Self {
        Self {
            code: "capability_unavailable",
            reason: reason.to_owned(),
            required_additional_fds: None,
            observed_open_fds: None,
            observed_soft_fd_limit: None,
            fd_limit_source: None,
        }
    }
}

pub(crate) fn preflight(
    implementation: Implementation,
    scenario: Scenario,
) -> Result<Option<Unsupported>, String> {
    let batch = scenario.batch_size();
    if batch == 0 {
        return Ok(None);
    }
    let required = u64::try_from(batch)
        .map_err(display)?
        .checked_mul(match implementation {
            Implementation::Zio => 3,
            Implementation::ZioBorrowed | Implementation::Mio | Implementation::Polling => 2,
        })
        .and_then(|value| value.checked_add(8))
        .ok_or_else(|| "descriptor requirement overflow".to_owned())?;
    let probe = FdProbe::discover();
    let Some(open) = probe.count().and_then(|value| u64::try_from(value).ok()) else {
        return Ok(None);
    };
    let Some((limit, source)) = soft_limit() else {
        return Ok(None);
    };
    if open.saturating_add(required) <= limit {
        return Ok(None);
    }
    Ok(Some(Unsupported {
        code: "insufficient_fd_limit",
        reason: format!(
            "scenario needs {required} additional descriptors with {open} already open, above soft limit {limit}"
        ),
        required_additional_fds: Some(required),
        observed_open_fds: Some(open),
        observed_soft_fd_limit: Some(limit),
        fd_limit_source: Some(source),
    }))
}

fn soft_limit() -> Option<(u64, &'static str)> {
    linux_limit()
        .map(|limit| (limit, "/proc/self/limits"))
        .or_else(|| shell_limit().map(|limit| (limit, "sh_ulimit_n")))
}

fn linux_limit() -> Option<u64> {
    let limits = fs::read_to_string("/proc/self/limits").ok()?;
    for line in limits.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.get(0..3) == Some(&["Max", "open", "files"])
            && let Some(value) = fields.get(3)
        {
            return parse_limit(value);
        }
    }
    None
}

fn shell_limit() -> Option<u64> {
    let output = Command::new("sh").args(["-c", "ulimit -n"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    parse_limit(value.trim())
}

pub(super) fn parse_limit(value: &str) -> Option<u64> {
    if value == "unlimited" {
        Some(u64::MAX)
    } else {
        value.parse().ok()
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
