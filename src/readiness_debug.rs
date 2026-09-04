//! Symbolic readiness formatting.

use core::fmt;

use super::Readiness;

impl fmt::Debug for Readiness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return formatter.write_str("EMPTY");
        }
        let mut separator = "";
        for (present, name) in [
            (self.is_readable(), "READABLE"),
            (self.is_writable(), "WRITABLE"),
            (self.is_read_closed(), "READ_CLOSED"),
            (self.is_write_closed(), "WRITE_CLOSED"),
            (self.is_error(), "ERROR"),
        ] {
            if present {
                formatter.write_str(separator)?;
                formatter.write_str(name)?;
                separator = " | ";
            }
        }
        Ok(())
    }
}
