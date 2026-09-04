//! Checked-in performance catalog synchronization.

use crate::Implementation;

use super::scenario::Scenario;

const CATALOG: &str = include_str!("../../perf-catalog.json");

#[test]
fn checked_in_catalog_matches_the_benchmark_model() {
    assert_eq!(CATALOG, render());
}

fn render() -> String {
    let candidates = strings(Implementation::ALL.into_iter().map(Implementation::name));
    let scenarios = strings(Scenario::ALL.into_iter().map(Scenario::name));
    let excluded = pairs(Implementation::ALL.into_iter().flat_map(|implementation| {
        Scenario::ALL
            .into_iter()
            .filter(move |scenario| !scenario.supports(implementation))
            .map(move |scenario| (implementation.name(), scenario.name()))
    }));
    let unsupported = triples(
        Scenario::ALL
            .into_iter()
            .filter(|scenario| scenario.requires_polling_level_support())
            .map(|scenario| ("polling", scenario.name(), "capability_unavailable")),
    );
    format!(
        "{{\"schema\":\"zio.performance-catalog.v1\",\"candidates\":[{candidates}],\"scenarios\":[{scenarios}],\"excluded_pairs\":[{excluded}],\"known_unsupported\":[{unsupported}],\"samples\":{{\"timing\":96,\"allocation\":12}},\"build_profile\":\"release\"}}\n"
    )
}

fn strings(values: impl Iterator<Item = &'static str>) -> String {
    values
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(",")
}

fn pairs(values: impl Iterator<Item = (&'static str, &'static str)>) -> String {
    values
        .map(|(left, right)| format!("[\"{left}\",\"{right}\"]"))
        .collect::<Vec<_>>()
        .join(",")
}

fn triples(values: impl Iterator<Item = (&'static str, &'static str, &'static str)>) -> String {
    values
        .map(|(first, second, third)| format!("[\"{first}\",\"{second}\",\"{third}\"]"))
        .collect::<Vec<_>>()
        .join(",")
}
