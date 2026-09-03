//! Delivery-mode classification.

use super::Mode;

#[test]
fn only_one_shot_disarms_after_delivery() {
    assert!(!Mode::Level.is_one_shot());
    assert!(Mode::OneShot.is_one_shot());
}
