use cf_core::{dates, fmt, money, names, parse, text, words};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value;

fn decimal(value: &Value) -> Option<Decimal> {
    match value {
        Value::Null => None,
        Value::String(text) => text.parse().ok(),
        Value::Number(number) => number.to_string().parse().ok(),
        _ => None,
    }
}

fn output_text(value: &Value) -> &str {
    value
        .as_str()
        .unwrap_or_else(|| value["ok"].as_str().unwrap_or_default())
}

#[test]
fn formatting_fixture_matches_core_ports() {
    let fixture = cf_testkit::fixture("formatting");
    for (key, render) in [
        (
            "fmt_currency",
            fmt::fmt_currency as fn(Option<Decimal>) -> String,
        ),
        ("fmt_number", fmt::fmt_number),
        ("fmt_vat", fmt::fmt_vat),
    ] {
        for case in fixture[key].as_array().unwrap() {
            if case["input"].is_null() {
                assert_eq!(render(None), case["output"].as_str().unwrap(), "{key}");
                continue;
            }
            let Some(number) = decimal(&case["input"]) else {
                continue;
            };
            assert_eq!(
                render(Some(number)),
                case["output"].as_str().unwrap(),
                "{key} input={:?}",
                case["input"]
            );
        }
    }

    for case in fixture["safe_decimal"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let actual = parse::safe_decimal(input).map(|value| value.normalize().to_string());
        assert_eq!(
            actual.as_deref(),
            case["output"].as_str(),
            "safe_decimal({input:?})"
        );
    }
    for case in fixture["safe_int"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        assert_eq!(
            parse::safe_int(input),
            case["output"].as_i64(),
            "safe_int({input:?})"
        );
    }
    for case in fixture["dec_str"].as_array().unwrap() {
        let actual = parse::dec_str(decimal(&case["input"]));
        assert_eq!(actual, case["output"].as_str().unwrap());
    }
    for case in fixture["fmt_money"].as_array().unwrap() {
        let actual = fmt::fmt_money(decimal(&case["input"]).unwrap());
        assert_eq!(actual, output_text(&case["output"]));
    }
    for case in fixture["manual_number"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let actual = parse::manual_number(input, "Giá trị", case["money"].as_bool().unwrap());
        if let Some(expected) = case["result"]["ok"].as_str() {
            assert_eq!(
                actual.unwrap().normalize().to_string(),
                expected,
                "manual_number({input:?})"
            );
        } else {
            assert_eq!(
                actual.unwrap_err().to_string(),
                case["result"]["msg"].as_str().unwrap()
            );
        }
    }
    for case in fixture["unit_price_text"].as_array().unwrap() {
        let value: Decimal = case["input"].as_str().unwrap().parse().unwrap();
        assert_eq!(
            fmt::unit_price_text(value),
            case["output"].as_str().unwrap()
        );
    }
}

#[test]
fn date_fixture_matches_calendar_ports() {
    let fixture = cf_testkit::fixture("dates");
    for case in fixture["add_months"].as_array().unwrap() {
        let start = NaiveDate::parse_from_str(case["start"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        let expected =
            NaiveDate::parse_from_str(case["output"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        assert_eq!(
            dates::add_months(start, case["months"].as_i64().unwrap() as i32),
            expected
        );
    }
    for case in fixture["compute_end_date"].as_array().unwrap() {
        let start = NaiveDate::parse_from_str(case["start"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        let expected =
            NaiveDate::parse_from_str(case["output"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        assert_eq!(
            dates::compute_end_date(start, case["months"].as_i64().unwrap() as i32),
            expected
        );
    }
    for case in fixture["months_between"].as_array().unwrap() {
        let start = NaiveDate::parse_from_str(case["start"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        let end = NaiveDate::parse_from_str(case["end"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        assert_eq!(
            dates::months_between(start, end),
            case["output"].as_i64().unwrap() as i32
        );
    }
    for case in fixture["parse_date_input"].as_array().unwrap() {
        let input = case["input"].as_str();
        let actual = dates::parse_date_input(input);
        if let Some(expected) = case["result"]["ok"].as_str() {
            assert_eq!(
                actual.unwrap().map(|date| date.to_string()).as_deref(),
                Some(expected)
            );
        } else if case["result"]["err"].is_null() {
            assert_eq!(actual.unwrap(), None);
        } else {
            assert_eq!(
                actual.unwrap_err().to_string(),
                case["result"]["msg"].as_str().unwrap()
            );
        }
    }
    for case in fixture["format"].as_array().unwrap() {
        let date = case["input"]
            .as_str()
            .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap());
        assert_eq!(dates::format_date_vn(date), case["vn"].as_str().unwrap());
        let (d, m, y) = dates::split_date_parts(date);
        assert_eq!(d, case["parts"]["day"].as_str().unwrap());
        assert_eq!(m, case["parts"]["month"].as_str().unwrap());
        assert_eq!(y, case["parts"]["year"].as_str().unwrap());
    }
}

#[test]
fn amount_to_words_fixture_and_text_normalization_match() {
    let fixture = cf_testkit::fixture("amount_to_words");
    for case in fixture["cases"].as_array().unwrap() {
        let Ok(value) = case["input"].as_str().unwrap().parse::<u64>() else {
            continue;
        };
        if case["words"]["ok"].is_null() {
            continue;
        }
        assert_eq!(
            words::amount_to_words(value),
            output_text(&case["words"]),
            "{value}"
        );
        assert_eq!(
            words::amount_to_words_chan(value),
            output_text(&case["chan"]),
            "{value}"
        );
    }
    assert_eq!(text::strip_diacritics("Đồng Tâm"), "Dong Tam");
    assert_eq!(text::strip_diacritics("Ứ ơ"), "U o");
    assert_eq!(text::search_key("Trạm Y Tế"), "tram y te");
    assert_eq!(
        money::to_decimal("1E+3", "số").unwrap(),
        Decimal::from(1000)
    );
    assert_eq!(names::safe_number_part("01/ABC/2026"), "01_ABC_2026");

    let kb01 = cf_testkit::kb("KB-01_amount_to_words");
    for case in kb01.as_array().unwrap() {
        let value: u64 = case["input"].as_str().unwrap().parse().unwrap();
        assert_eq!(
            words::amount_to_words(value),
            case["rust_expected"]["words"].as_str().unwrap()
        );
        assert_eq!(
            words::amount_to_words_chan(value),
            case["rust_expected"]["chan"].as_str().unwrap()
        );
    }
    assert_eq!(
        words::amount_to_words_dec(Decimal::new(123456, 2)).unwrap(),
        "Một nghìn, hai trăm ba mươi bốn đồng"
    );
    assert_eq!(
        words::amount_to_words_dec(Decimal::from(-1))
            .unwrap_err()
            .to_string(),
        "Không hỗ trợ số âm"
    );

    let kb19 = cf_testkit::kb("KB-19_manual_quotation_filename");
    for case in kb19.as_array().unwrap() {
        let layout = case["layout"].as_str().unwrap();
        let quote_date = case["quote_date"].as_str().unwrap();
        let raw = format!("BaoGia_{layout}_{quote_date}.docx");
        assert_eq!(
            names::windows_safe_filename(&raw),
            case["rust_expected"].as_str().unwrap()
        );
    }
}
