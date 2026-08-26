//! Reusable strict value parsing for benchmark command-line options.

use std::ffi::OsString;

use crate::Implementation;

pub(crate) fn number(
    args: &mut impl Iterator<Item = OsString>,
    flag: &'static str,
    maximum: usize,
) -> Result<usize, String> {
    let value = text(args, flag)?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{flag} requires an integer from 1 through {maximum}"))?;
    if parsed == 0 || parsed > maximum {
        return Err(format!(
            "{flag} must be between 1 and {maximum}; received {parsed}"
        ));
    }
    Ok(parsed)
}

pub(crate) fn number_u64(
    args: &mut impl Iterator<Item = OsString>,
    flag: &'static str,
    maximum: u64,
) -> Result<u64, String> {
    let value = text(args, flag)?;
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{flag} requires an integer from 1 through {maximum}"))?;
    if parsed == 0 || parsed > maximum {
        return Err(format!(
            "{flag} must be between 1 and {maximum}; received {parsed}"
        ));
    }
    Ok(parsed)
}

pub(crate) fn text(
    args: &mut impl Iterator<Item = OsString>,
    flag: &'static str,
) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))?
        .into_string()
        .map_err(|_| format!("{flag} requires a valid UTF-8 value"))
}

pub(crate) fn implementation(value: &str) -> Result<Implementation, String> {
    match value {
        "zio" => Ok(Implementation::Zio),
        "mio" => Ok(Implementation::Mio),
        "polling" => Ok(Implementation::Polling),
        _ => Err(format!(
            "unknown implementation `{value}`; expected zio, mio, or polling"
        )),
    }
}

pub(crate) fn set_flag(slot: &mut bool, flag: &'static str) -> Result<(), String> {
    if *slot {
        Err(format!("duplicate {flag}"))
    } else {
        *slot = true;
        Ok(())
    }
}

pub(crate) fn set_value<T>(
    slot: &mut Option<T>,
    value: T,
    flag: &'static str,
) -> Result<(), String> {
    if slot.is_some() {
        Err(format!("duplicate {flag}"))
    } else {
        *slot = Some(value);
        Ok(())
    }
}
