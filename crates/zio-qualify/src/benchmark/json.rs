//! Allocation-conscious JSON encoding for NDJSON receipts.

pub(crate) fn string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            value if value <= '\u{1f}' => control(output, value),
            value => output.push(value),
        }
    }
    output.push('"');
}

pub(crate) fn key(output: &mut String, value: &str) {
    string(output, value);
    output.push(':');
}

pub(crate) fn field_string(output: &mut String, name: &str, value: &str, comma: bool) {
    key(output, name);
    string(output, value);
    if comma {
        output.push(',');
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "receipt numbers are Copy and the direct value API keeps call sites declarative"
)]
pub(crate) fn field_number<T: ToString>(output: &mut String, name: &str, value: T, comma: bool) {
    key(output, name);
    output.push_str(&value.to_string());
    if comma {
        output.push(',');
    }
}

pub(crate) fn array_u128(output: &mut String, values: impl Iterator<Item = u128>) {
    output.push('[');
    let mut first = true;
    for value in values {
        separator(output, &mut first);
        output.push_str(&value.to_string());
    }
    output.push(']');
}

pub(crate) fn array_u64(output: &mut String, values: impl Iterator<Item = u64>) {
    output.push('[');
    let mut first = true;
    for value in values {
        separator(output, &mut first);
        output.push_str(&value.to_string());
    }
    output.push(']');
}

pub(crate) fn array_i64(output: &mut String, values: impl Iterator<Item = i64>) {
    output.push('[');
    let mut first = true;
    for value in values {
        separator(output, &mut first);
        output.push_str(&value.to_string());
    }
    output.push(']');
}

pub(crate) fn array_usize(output: &mut String, values: impl Iterator<Item = usize>) {
    output.push('[');
    let mut first = true;
    for value in values {
        separator(output, &mut first);
        output.push_str(&value.to_string());
    }
    output.push(']');
}

fn separator(output: &mut String, first: &mut bool) {
    if *first {
        *first = false;
    } else {
        output.push(',');
    }
}

fn control(output: &mut String, value: char) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let byte = value as usize;
    output.push_str("\\u00");
    output.push(char::from(HEX[(byte >> 4) & 0x0f]));
    output.push(char::from(HEX[byte & 0x0f]));
}
