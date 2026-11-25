use cf_core::{money, pricing};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value;

fn dec(value: &Value) -> Option<Decimal> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.parse().unwrap()),
        Value::Number(number) => Some(number.to_string().parse().unwrap()),
        _ => None,
    }
}

fn result_decimal(actual: Result<Decimal, cf_core::error::CoreError>, expected: &Value) {
    if let Some(value) = expected["ok"].as_str() {
        let actual = actual.unwrap();
        let expected: Decimal = value.parse().unwrap();
        assert_eq!(actual.round_dp(27), expected.round_dp(27));
    } else {
        assert_eq!(
            actual.unwrap_err().to_string(),
            expected["msg"].as_str().unwrap()
        );
    }
}

#[test]
fn pricing_scalar_fixtures_match() {
    let fixture = cf_testkit::fixture("pricing");
    for case in fixture["round_money"].as_array().unwrap() {
        let raw = case["input"].as_str().unwrap();
        let actual = money::to_decimal(raw, "giá trị").map(money::round_half_up);
        if let Some(expected) = case["output"]["ok"].as_str() {
            assert_eq!(actual.unwrap().normalize().to_string(), expected);
        } else {
            assert_eq!(
                actual.unwrap_err().to_string(),
                case["output"]["msg"].as_str().unwrap()
            );
        }
    }
    for case in fixture["normalize_quantity"].as_array().unwrap() {
        result_decimal(
            pricing::normalize_quantity(
                case["raw"].as_str().unwrap(),
                dec(&case["min_quantity"]),
                dec(&case["step"]),
            ),
            &case["result"],
        );
    }
    for case in fixture["period_count"].as_array().unwrap() {
        result_decimal(
            pricing::period_count(
                case["months"].to_string().trim_matches('"'),
                case["price_period_months"].as_str().or_else(|| {
                    (!case["price_period_months"].is_null()).then(|| {
                        Box::leak(case["price_period_months"].to_string().into_boxed_str()) as &str
                    })
                }),
            ),
            &case["result"],
        );
    }
    for case in fixture["unit_price_with_vat"].as_array().unwrap() {
        let price = dec(&case["price"]).unwrap();
        let vat = dec(&case["vat"]).unwrap();
        assert_eq!(
            pricing::unit_price_with_vat(price, vat)
                .normalize()
                .to_string(),
            case["output"].as_str().unwrap()
        );
    }
}

#[test]
fn legacy_amounts_preserve_explicit_zero_vat_price() {
    let amounts = pricing::legacy_amounts(
        Some(Decimal::from(2)),
        Some(Decimal::ONE),
        Some(Decimal::from(100)),
        Some(Decimal::from(10)),
        Some(Decimal::ZERO),
        Some(Decimal::from(2)),
        None,
    );
    assert_eq!(amounts.unit_price_vat, Some(Decimal::ZERO));
    assert_eq!(amounts.total_amount, Some(Decimal::ZERO));
}

#[test]
fn blank_item_price_and_vat_use_product_defaults() {
    let rule = pricing::ProductRule {
        id: 2,
        name: "Supplement".into(),
        kind: "addon".into(),
        group_code: Some("G1".into()),
        is_active: true,
        unit_label: "unit".into(),
        default_price: Some(Decimal::from(100)),
        default_vat_rate: Some(Decimal::from(10)),
        price_period_months: Some(Decimal::ONE),
        min_quantity: None,
        quantity_step: None,
        quantity_source: None,
    };
    let lines = pricing::build_lines(
        &[pricing::ItemInput {
            product_id: 2,
            description: String::new(),
            quantity: "1".into(),
            unit_price_pre_vat: Some(String::new()),
            vat_rate: Some("   ".into()),
            amount_pre_vat_override: None,
            note: String::new(),
        }],
        |_| Some(rule.clone()),
        1,
        false,
    )
    .unwrap();
    assert_eq!(lines[0].unit_price_pre_vat, Decimal::from(100));
    assert_eq!(lines[0].vat_rate, Decimal::from(10));
}

#[test]
fn price_line_fixtures_match() {
    let fixture = cf_testkit::fixture("pricing");
    for case in fixture["price_line"].as_array().unwrap() {
        let input = &case["input"];
        let actual = pricing::price_line(pricing::LineInput {
            description: input["description"].as_str().unwrap().to_owned(),
            unit_label: input["unit_label"].as_str().unwrap_or_default().to_owned(),
            quantity: dec(&input["quantity"]).unwrap_or(Decimal::ONE),
            months: dec(&input["months"]).unwrap_or(Decimal::ONE),
            unit_price_pre_vat: dec(&input["unit_price_pre_vat"]).unwrap_or(Decimal::ZERO),
            vat_rate: dec(&input["vat_rate"]).unwrap_or(Decimal::ZERO),
            price_period_months: dec(&input["price_period_months"]).unwrap_or(Decimal::ONE),
            amount_pre_vat_override: dec(&input["amount_pre_vat_override"]),
            product_id: input["product_id"].as_i64(),
            note: input["note"].as_str().unwrap_or_default().to_owned(),
        });
        if let Some(expected) = case["result"]["ok"].as_object() {
            let line = actual.unwrap();
            for (key, value) in expected {
                let actual = match key.as_str() {
                    "description" => line.description.clone(),
                    "unit_label" => line.unit_label.clone(),
                    "quantity" => line.quantity.normalize().to_string(),
                    "period_count" => line.period_count.normalize().to_string(),
                    "unit_price_pre_vat" => line.unit_price_pre_vat.normalize().to_string(),
                    "vat_rate" => line.vat_rate.normalize().to_string(),
                    "amount_pre_vat" => line.amount_pre_vat.normalize().to_string(),
                    "vat_amount" => line.vat_amount.normalize().to_string(),
                    "amount_total" => line.amount_total.normalize().to_string(),
                    "product_id" => line.product_id.map(|n| n.to_string()).unwrap_or_default(),
                    "note" => line.note.clone(),
                    _ => continue,
                };
                if value.is_null() {
                    assert!(actual.is_empty(), "{key}");
                } else {
                    assert_eq!(
                        actual,
                        value.as_str().unwrap_or_else(|| value.to_string().leak()),
                        "{key}"
                    );
                }
            }
        } else {
            assert_eq!(
                actual.unwrap_err().to_string(),
                case["result"]["msg"].as_str().unwrap()
            );
        }
    }
}

#[test]
fn addon_months_fixture_matches() {
    let fixture = cf_testkit::fixture("pricing");
    for case in fixture["addon_months"].as_array().unwrap() {
        let start = NaiveDate::parse_from_str(case["start"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        let end =
            NaiveDate::parse_from_str(case["main_end"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        let actual = pricing::addon_months(start, end);
        if let Some(expected) = case["result"]["ok"].as_i64() {
            assert_eq!(actual.unwrap(), expected as i32);
        } else {
            assert_eq!(
                actual.unwrap_err().to_string(),
                case["result"]["msg"].as_str().unwrap()
            );
        }
    }
}

#[test]
fn sum_lines_legacy_auto_and_build_lines_match() {
    let fixture = cf_testkit::fixture("pricing");
    let sum_case = &fixture["sum_lines"];
    let lines: Vec<pricing::Line> = sum_case["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| pricing::Line {
            description: l["description"].as_str().unwrap().to_owned(),
            unit_label: l["unit_label"].as_str().unwrap_or_default().to_owned(),
            quantity: dec(&l["quantity"]).unwrap(),
            period_count: dec(&l["period_count"]).unwrap(),
            unit_price_pre_vat: dec(&l["unit_price_pre_vat"]).unwrap(),
            vat_rate: dec(&l["vat_rate"]).unwrap(),
            amount_pre_vat: dec(&l["amount_pre_vat"]).unwrap(),
            vat_amount: dec(&l["vat_amount"]).unwrap(),
            amount_total: dec(&l["amount_total"]).unwrap(),
            product_id: l["product_id"].as_i64(),
            note: l["note"].as_str().unwrap_or_default().to_owned(),
        })
        .collect();
    let totals = pricing::sum_lines(&lines).unwrap();
    assert_eq!(totals.pre_vat, dec(&sum_case["totals"]["pre_vat"]).unwrap());
    assert_eq!(totals.vat, dec(&sum_case["totals"]["vat"]).unwrap());
    assert_eq!(totals.total, dec(&sum_case["totals"]["total"]).unwrap());
    assert_eq!(
        totals.amount_text,
        sum_case["totals"]["amount_text"].as_str().unwrap()
    );
    assert_eq!(
        pricing::describe(&lines, &totals),
        sum_case["describe"].as_str().unwrap()
    );
    assert_eq!(
        pricing::sum_lines(&[]).unwrap_err().to_string(),
        fixture["empty_sum"]["msg"].as_str().unwrap()
    );

    // T1.31 legacy_amounts
    let legacy = pricing::legacy_amounts(
        Some(Decimal::from(2)),
        Some(Decimal::from(12)),
        Some(Decimal::from(440_909)),
        Some(Decimal::from(10)),
        None,
        None,
        None,
    );
    assert_eq!(legacy.unit_price_vat, Some("484999.9".parse().unwrap()));
    assert_eq!(legacy.quantity, Some(Decimal::from(24)));
    assert_eq!(legacy.total_amount, Some("11639997.6".parse().unwrap()));
    assert_eq!(
        legacy.amount_text,
        "Mười một triệu, sáu trăm ba mươi chín nghìn, chín trăm chín mươi bảy đồng"
    );
    let kept = pricing::legacy_amounts(
        Some(Decimal::from(2)),
        Some(Decimal::from(12)),
        Some(Decimal::from(440_909)),
        Some(Decimal::from(10)),
        None,
        None,
        Some(Decimal::from(999_000)),
    );
    assert_eq!(kept.total_amount, Some(Decimal::from(999_000)));

    // T1.40 auto_pricing_description
    assert_eq!(
        pricing::auto_pricing_description(
            Some(Decimal::from(485_000)),
            2,
            &["A".into(), "B".into()],
            Some(12),
        ),
        "Số tiền = Đơn giá có VAT (485.000 VNĐ) × 2 cơ sở (A; B) × 12 tháng = 11.640.000 VNĐ"
    );
    assert_eq!(
        pricing::auto_pricing_description(Some(Decimal::from(485_000)), 1, &["A".into()], Some(12),),
        "Số tiền = Đơn giá có VAT (485.000 VNĐ) × 1 cơ sở × 12 tháng = 5.820.000 VNĐ"
    );
    assert_eq!(
        pricing::auto_pricing_description(None, 1, &[], Some(12)),
        ""
    );

    // T1.41 build_lines + default_quantity
    let main_g1 = pricing::ProductRule {
        id: 1,
        name: "Gói chính".into(),
        kind: "main".into(),
        group_code: Some("G1".into()),
        is_active: true,
        unit_label: "cơ sở".into(),
        default_price: Some(Decimal::from(500_000)),
        default_vat_rate: Some(Decimal::from(10)),
        price_period_months: Some(Decimal::from(12)),
        min_quantity: Some(Decimal::from(100)),
        quantity_step: Some(Decimal::from(50)),
        quantity_source: Some("records".into()),
    };
    let addon_g1 = pricing::ProductRule {
        id: 2,
        name: "Gói thêm G1".into(),
        kind: "addon".into(),
        group_code: Some("G1".into()),
        is_active: true,
        unit_label: "cơ sở".into(),
        default_price: Some(Decimal::from(100_000)),
        default_vat_rate: Some(Decimal::from(10)),
        price_period_months: Some(Decimal::from(12)),
        min_quantity: None,
        quantity_step: None,
        quantity_source: Some("units".into()),
    };
    let addon_g2 = pricing::ProductRule {
        id: 3,
        name: "Gói thêm G2".into(),
        kind: "addon".into(),
        group_code: Some("G2".into()),
        is_active: true,
        unit_label: "cơ sở".into(),
        default_price: Some(Decimal::from(100_000)),
        default_vat_rate: None,
        price_period_months: None,
        min_quantity: None,
        quantity_step: None,
        quantity_source: None,
    };
    let inactive = pricing::ProductRule {
        id: 4,
        name: "Gói cũ".into(),
        kind: "addon".into(),
        group_code: Some("G1".into()),
        is_active: false,
        unit_label: "cơ sở".into(),
        default_price: Some(Decimal::from(100_000)),
        default_vat_rate: None,
        price_period_months: None,
        min_quantity: None,
        quantity_step: None,
        quantity_source: None,
    };
    let lookup = |id: i64| match id {
        1 => Some(main_g1.clone()),
        2 => Some(addon_g1.clone()),
        3 => Some(addon_g2.clone()),
        4 => Some(inactive.clone()),
        _ => None,
    };
    let item = |product_id: i64, qty: &str| pricing::ItemInput {
        product_id,
        description: String::new(),
        quantity: qty.into(),
        unit_price_pre_vat: None,
        vat_rate: None,
        amount_pre_vat_override: None,
        note: String::new(),
    };
    assert_eq!(pricing::default_quantity(&main_g1, Some(120), 2), "150");
    assert_eq!(pricing::default_quantity(&addon_g1, Some(120), 2), "2");
    assert_eq!(
        pricing::build_lines(&[item(2, "1")], lookup, 12, true)
            .unwrap_err()
            .to_string(),
        "Dòng 1 phải là gói chính."
    );
    assert_eq!(
        pricing::build_lines(&[item(1, "1"), item(1, "1")], lookup, 12, true)
            .unwrap_err()
            .to_string(),
        "Dòng 2: chỉ dòng đầu tiên được là gói chính."
    );
    assert_eq!(
        pricing::build_lines(&[item(1, "1"), item(3, "1")], lookup, 12, true)
            .unwrap_err()
            .to_string(),
        "Dòng 2: gói bổ sung phải cùng nhóm với gói chính."
    );
    assert_eq!(
        pricing::build_lines(&[item(1, "1"), item(4, "1")], lookup, 12, true)
            .unwrap_err()
            .to_string(),
        "Dòng 2: gói 'Gói cũ' đã ngừng bán."
    );
    let ok = pricing::build_lines(&[item(1, "120"), item(2, "2")], lookup, 12, true).unwrap();
    assert_eq!(ok.len(), 2);
    assert_eq!(ok[0].quantity, Decimal::from(150));
    assert_eq!(ok[0].unit_price_pre_vat, Decimal::from(500_000));
}
