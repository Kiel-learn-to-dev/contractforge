use cf_core::{lifecycle, names, numbering};
use chrono::{NaiveDate, NaiveDateTime};
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn names_fixture_matches_slug_sanitizer_and_filename_ports() {
    let fixture = cf_testkit::fixture("names");
    for case in fixture["slug"].as_array().unwrap() {
        assert_eq!(
            names::make_contract_slug(case["input"].as_str().unwrap()),
            case["output"].as_str().unwrap()
        );
    }
    for case in fixture["sanitize"].as_array().unwrap() {
        assert_eq!(
            names::sanitize_for_filename(
                case["input"].as_str().unwrap(),
                case["max_len"].as_u64().unwrap() as usize
            ),
            case["output"].as_str().unwrap()
        );
    }
    for case in fixture["output_filename"].as_array().unwrap() {
        let ts = NaiveDateTime::parse_from_str(case["ts"].as_str().unwrap(), "%Y-%m-%dT%H:%M:%S")
            .unwrap();
        assert_eq!(
            names::make_output_filename(
                case["contract_number"].as_str().unwrap(),
                case["customer_code"].as_str().unwrap(),
                case["contract_type"].as_str().unwrap(),
                ts
            ),
            case["output"].as_str().unwrap()
        );
    }
}

#[test]
fn numbering_fixture_matches_validation_building_and_sequences() {
    let fixture = cf_testkit::fixture("numbering");
    for case in fixture["validate_format"].as_array().unwrap() {
        let actual = numbering::validate_format(case["input"].as_str().unwrap());
        assert_result_text(actual.map(str::to_owned), &case["result"]);
    }
    for case in fixture["build_and_like"].as_array().unwrap() {
        if case.get("seq").is_some() {
            let actual = numbering::build_number(
                case["seq"].as_str().unwrap(),
                case["slug"].as_str().unwrap(),
                case["format"].as_str(),
                case["year"].as_i64().unwrap() as i32,
            );
            assert_result_text(actual, &case["result"]);
        }
    }
    for case in fixture["next_seq"].as_array().unwrap() {
        let used: BTreeSet<u32> = case["used"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(numbering::next_seq(&used), case["output"].as_str().unwrap());
    }

    let kb02 = cf_testkit::kb("KB-02_numbering_next_seq");
    for case in kb02.as_array().unwrap() {
        let existing: Vec<&str> = case["existing_numbers_of_customer"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let actual = numbering::next_number(
            case["format"].as_str().unwrap(),
            case["slug"].as_str().unwrap(),
            case["year"].as_i64().unwrap() as i32,
            &existing,
        )
        .unwrap();
        assert_eq!(
            actual,
            case["rust_expected"].as_str().unwrap(),
            "{}",
            case["case"].as_str().unwrap()
        );
    }

    for case in fixture["addendum"].as_array().unwrap() {
        let fmt = case["format"].as_str().unwrap();
        assert_result_text(
            numbering::validate_addendum_format(fmt).map(str::to_owned),
            &case["validate"],
        );
        if let Some(build) = case.get("build") {
            assert_result_text(
                numbering::build_addendum_number("01/ABC/2026", "02", Some(fmt)),
                build,
            );
        }
        if let Some(parse_cases) = case["parse"].as_array() {
            for p in parse_cases {
                let actual =
                    numbering::parse_addendum_seq(p["number"].as_str().unwrap(), Some(fmt));
                assert_eq!(actual, p["output"]["ok"].as_u64().map(|v| v as u32));
            }
        }
    }
    let used_addendum: BTreeSet<u32> = [1, 3].into_iter().collect();
    assert_eq!(numbering::next_addendum_seq(&used_addendum), "02");
}

fn assert_result_text(actual: Result<String, cf_core::error::CoreError>, expected: &Value) {
    if let Some(value) = expected["ok"].as_str() {
        assert_eq!(actual.unwrap(), value);
    } else {
        assert_eq!(
            actual.unwrap_err().to_string(),
            expected["msg"].as_str().unwrap()
        );
    }
}

#[test]
fn lifecycle_fixture_transition_matrix_and_labels_match() {
    let fixture = cf_testkit::fixture("lifecycle");
    for case in fixture["transition_matrix"].as_array().unwrap() {
        let from = lifecycle::parse_status(case["from"].as_str().unwrap()).unwrap();
        let actual = lifecycle::validate_transition(
            &from,
            case["to"].as_str().unwrap(),
            lifecycle::Evidence {
                has_invoice: case["has_invoice"].as_bool().unwrap(),
                has_payment_slip: case["has_slip"].as_bool().unwrap(),
            },
        );
        if let Some(expected) = case["result"]["ok"].as_str() {
            assert_eq!(actual.unwrap().as_str(), expected);
        } else {
            assert_eq!(
                actual.unwrap_err().to_string(),
                case["result"]["msg"].as_str().unwrap()
            );
        }
    }
    for (status, data) in fixture["status_labels"].as_object().unwrap() {
        assert_eq!(lifecycle::status_color(status), data[0].as_str().unwrap());
        assert_eq!(lifecycle::status_label(status), data[1].as_str().unwrap());
    }

    let sets = &fixture["sets"];
    fn to_strs(slice: &[lifecycle::ContractStatus]) -> Vec<&str> {
        slice.iter().map(|s| s.as_str()).collect()
    }
    let expected_strs = |key: &str| -> Vec<&str> {
        sets[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect()
    };
    assert_eq!(
        to_strs(lifecycle::ACTIVE_LIFECYCLE),
        expected_strs("ACTIVE_LIFECYCLE_STATUSES")
    );
    assert_eq!(
        to_strs(lifecycle::RENEWAL_COUNTS),
        expected_strs("RENEWAL_COUNTS_STATUSES")
    );
    assert_eq!(
        to_strs(lifecycle::IN_SERVICE),
        expected_strs("IN_SERVICE_STATUSES")
    );
    assert_eq!(to_strs(lifecycle::UNPAID), expected_strs("UNPAID_STATUSES"));
    assert_eq!(
        to_strs(lifecycle::DEPRECATED),
        expected_strs("DEPRECATED_STATUSES")
    );
    assert!(!lifecycle::SELECTABLE.contains(&lifecycle::ContractStatus::ExpiringSoon));
    assert!(!lifecycle::SELECTABLE.contains(&lifecycle::ContractStatus::Active));

    for case in fixture["expiry"].as_array().unwrap() {
        let end = case["end_date"]
            .as_str()
            .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap());
        let today = NaiveDate::parse_from_str(case["today"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        assert_eq!(
            lifecycle::days_to_expiry(end, today),
            case["days_to_expiry"].as_i64()
        );
        let expected_bucket = case["bucket"].as_str().map(|b| match b {
            "expired" => lifecycle::Bucket::Expired,
            "critical" => lifecycle::Bucket::Critical,
            "warning" => lifecycle::Bucket::Warning,
            "info" => lifecycle::Bucket::Info,
            other => panic!("unexpected bucket: {other}"),
        });
        assert_eq!(lifecycle::expiry_bucket(end, today), expected_bucket);
        assert_eq!(
            lifecycle::is_expiring_soon(end, today),
            case["is_expiring_soon"].as_bool().unwrap()
        );
    }
}
