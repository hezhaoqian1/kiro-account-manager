use super::*;

#[test]
fn ensure_citation_target_supported_accepts_location_without_guessing_range() {
    assert_eq!(
        ensure_citation_target_supported(&serde_json::json!({ "location": 6 })),
        Some(())
    );
}
