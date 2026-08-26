//! Host, toolchain, and source-state provenance collection.

use std::{env, fs, process::Command, time::SystemTime};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Metadata {
    pub(crate) git_sha: String,
    pub(crate) git_sha_source: &'static str,
    pub(crate) git_dirty: Option<bool>,
    pub(crate) os: &'static str,
    pub(crate) os_version: String,
    pub(crate) arch: &'static str,
    pub(crate) cpu: String,
    pub(crate) rustc: String,
    pub(crate) qualify_version: &'static str,
    pub(crate) build_profile: &'static str,
    pub(crate) rustflags: String,
    pub(crate) recorded_unix_ms: u128,
}

impl Metadata {
    pub(crate) fn collect() -> Self {
        let (git_sha, git_sha_source) = git_sha();
        Self {
            git_sha,
            git_sha_source,
            git_dirty: git_dirty(),
            os: std::env::consts::OS,
            os_version: command_output("uname".as_ref(), &["-sr"])
                .and_then(|value| cleaned(&value))
                .unwrap_or_else(|| "unavailable".to_owned()),
            arch: std::env::consts::ARCH,
            cpu: cpu(),
            rustc: command_output(
                env::var_os("RUSTC")
                    .as_deref()
                    .unwrap_or_else(|| "rustc".as_ref()),
                &["--version", "--verbose"],
            )
            .unwrap_or_else(|| "unavailable".to_owned()),
            qualify_version: env!("CARGO_PKG_VERSION"),
            build_profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            rustflags: rustflags(),
            recorded_unix_ms: SystemTime::UNIX_EPOCH
                .elapsed()
                .map_or(0, |elapsed| elapsed.as_millis()),
        }
    }
}

fn rustflags() -> String {
    env::var("RUSTFLAGS")
        .ok()
        .and_then(|value| cleaned(&value))
        .or_else(|| {
            env::var("CARGO_ENCODED_RUSTFLAGS")
                .ok()
                .and_then(|value| cleaned(&value.replace('\u{1f}', " ")))
        })
        .unwrap_or_else(|| "unavailable".to_owned())
}

fn git_sha() -> (String, &'static str) {
    for (name, source) in [
        ("ZIO_PERF_GIT_SHA", "ZIO_PERF_GIT_SHA"),
        ("GITHUB_SHA", "GITHUB_SHA"),
    ] {
        if let Ok(value) = env::var(name)
            && let Some(value) = cleaned(&value)
        {
            return (value, source);
        }
    }
    command_output("git".as_ref(), &["rev-parse", "HEAD"])
        .and_then(|value| cleaned(&value))
        .map_or_else(
            || ("unavailable".to_owned(), "unavailable"),
            |value| (value, "git_rev_parse"),
        )
}

fn git_dirty() -> Option<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .ok()?;
    output.status.success().then_some(!output.stdout.is_empty())
}

fn cpu() -> String {
    if let Ok(value) = env::var("PROCESSOR_IDENTIFIER")
        && let Some(value) = cleaned(&value)
    {
        return value;
    }
    if let Ok(contents) = fs::read_to_string("/proc/cpuinfo") {
        for line in contents.lines() {
            if let Some((name, value)) = line.split_once(':')
                && matches!(name.trim(), "model name" | "Hardware")
                && let Some(value) = cleaned(value)
            {
                return value;
            }
        }
    }
    for name in ["machdep.cpu.brand_string", "hw.model"] {
        if let Some(value) =
            command_output("sysctl".as_ref(), &["-n", name]).and_then(|value| cleaned(&value))
        {
            return value;
        }
    }
    "unavailable".to_owned()
}

fn command_output(program: &std::ffi::OsStr, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn cleaned(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
pub(crate) fn fixture() -> Metadata {
    Metadata {
        git_sha: "0123456789abcdef".to_owned(),
        git_sha_source: "fixture",
        git_dirty: Some(false),
        os: "test-os",
        os_version: "test-os 1".to_owned(),
        arch: "test-arch",
        cpu: "test-cpu".to_owned(),
        rustc: "rustc 1.88.0\nrelease: 1.88.0".to_owned(),
        qualify_version: "0.0.1-dev.1",
        build_profile: "test-profile",
        rustflags: "-C target-cpu=test".to_owned(),
        recorded_unix_ms: 1_234,
    }
}
