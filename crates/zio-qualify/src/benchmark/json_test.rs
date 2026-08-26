//! JSON receipt encoder tests.

use super::json::string;

#[test]
fn escapes_ndjson_control_characters() -> Result<(), String> {
    let mut output = String::new();
    string(&mut output, "a\n\"\\\u{1f}");
    if output == "\"a\\n\\\"\\\\\\u001f\"" {
        Ok(())
    } else {
        Err(format!("unexpected JSON string {output}"))
    }
}
