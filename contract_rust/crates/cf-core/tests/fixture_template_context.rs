use cf_core::{customer, lifecycle, pricing, render_context, template_rules};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::BTreeMap;

fn role(value: &str) -> template_rules::Role {
    match value {
        "addon" => template_rules::Role::Addon,
        "renewal" => template_rules::Role::Renewal,
        "attachment" => template_rules::Role::Attachment,
        "quotation" => template_rules::Role::Quotation,
        "quote_request" => template_rules::Role::QuoteRequest,
        _ => template_rules::Role::Contract,
    }
}

#[test]
fn template_rule_fixtures_match() {
    let fixture = cf_testkit::fixture("template_rules");
    assert_eq!(
        template_rules::CONTRACT_TYPES,
        fixture["CONTRACT_TYPES"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::as_str)
            .map(Option::unwrap)
            .collect::<Vec<_>>()
    );
    for case in fixture["has_role"].as_array().unwrap() {
        assert_eq!(
            template_rules::has_role(
                case["contract_type"].as_str().unwrap(),
                role(case["role"].as_str().unwrap())
            ),
            case["output"].as_bool().unwrap()
        );
    }
    for case in fixture["check_template_for"].as_array().unwrap() {
        let template = template_rules::TemplateInfo {
            id: 1,
            name: "Mẫu thử".into(),
            contract_type: case["contract_type"].as_str().unwrap().into(),
            product_group: case["template_group"].as_str().map(str::to_owned),
            is_active: true,
        };
        let actual = template_rules::check_template_for(
            &template,
            case["group"].as_str(),
            role(case["role"].as_str().unwrap()),
        );
        if let Some(ok) = case["result"].get("ok") {
            assert!(actual.is_ok(), "fixture expected {ok}");
        } else {
            assert_eq!(
                actual.unwrap_err().to_string(),
                case["result"]["msg"].as_str().unwrap()
            );
        }
    }
    for case in fixture["template_kind"].as_array().unwrap() {
        let placeholders = case["placeholders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            template_rules::template_kind(&placeholders, case["heading"].as_str().unwrap()),
            case["output"].as_str().unwrap()
        );
    }
    for case in fixture["preselected_attachment"].as_array().unwrap() {
        let template = template_rules::TemplateInfo {
            id: 1,
            name: "Mẫu thử".into(),
            contract_type: case["contract_type"].as_str().unwrap().into(),
            product_group: case["product_group"].as_str().map(str::to_owned),
            is_active: true,
        };
        assert_eq!(
            template_rules::preselected_attachment(&template),
            case["output"].as_bool().unwrap()
        );
    }
    for (kind, expected) in fixture["role_of_doc_kind"].as_object().unwrap() {
        let actual = template_rules::role_of_doc_kind(Some(kind));
        let actual = match actual {
            template_rules::Role::Contract => "contract",
            template_rules::Role::Addon => "addon",
            template_rules::Role::Renewal => "renewal",
            _ => "other",
        };
        assert_eq!(actual, expected.as_str().unwrap());
    }
}

#[test]
fn render_mapping_and_work_content_fixtures_match() {
    let fixture = cf_testkit::fixture("render_context");
    for (placeholder, canonical) in fixture["canonical_map"].as_object().unwrap() {
        assert_eq!(
            render_context::CANONICAL_MAP
                .iter()
                .find(|(key, _)| key == placeholder)
                .map(|(_, field)| *field),
            canonical.as_str()
        );
    }
    for (placeholder, field) in fixture["row_placeholders"].as_object().unwrap() {
        assert!(
            render_context::ROW_PLACEHOLDERS
                .contains(&(placeholder.as_str(), field.as_str().unwrap()))
        );
    }
    for case in std::iter::once(&fixture["template_mapping"]) {
        let input = case["input"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.as_str().unwrap().to_owned()))
            .collect::<BTreeMap<_, _>>();
        let mapping = case["field_mapping"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.as_str().unwrap().to_owned()))
            .collect::<BTreeMap<_, _>>();
        let expected = case["output"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.as_str().unwrap().to_owned()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            render_context::apply_template_mapping(
                &input,
                &render_context::TemplateMapping(mapping)
            ),
            expected
        );
    }
    for case in fixture["work_content"].as_array().unwrap() {
        let product = render_context::ProductView {
            name: case["product_name"].as_str().unwrap().into(),
            code: String::new(),
        };
        assert_eq!(
            render_context::work_content(Some(&product)),
            case["output"].as_str().unwrap()
        );
    }

    // T1.34 templates_for & default_template_for
    let templates = vec![
        template_rules::TemplateInfo {
            id: 1,
            name: "Mẫu dùng chung".into(),
            contract_type: "Hợp đồng mới".into(),
            product_group: None,
            is_active: true,
        },
        template_rules::TemplateInfo {
            id: 2,
            name: "Mẫu Z G1".into(),
            contract_type: "Hợp đồng mới".into(),
            product_group: Some("G1".into()),
            is_active: true,
        },
        template_rules::TemplateInfo {
            id: 3,
            name: "Mẫu A G1".into(),
            contract_type: "Hợp đồng mới".into(),
            product_group: Some("G1".into()),
            is_active: true,
        },
        template_rules::TemplateInfo {
            id: 4,
            name: "Mẫu G2".into(),
            contract_type: "Hợp đồng mới".into(),
            product_group: Some("G2".into()),
            is_active: true,
        },
        template_rules::TemplateInfo {
            id: 5,
            name: "Mẫu tắt".into(),
            contract_type: "Hợp đồng mới".into(),
            product_group: Some("G1".into()),
            is_active: false,
        },
    ];
    let filtered =
        template_rules::templates_for(&templates, Some("G1"), template_rules::Role::Contract, true);
    let ids: Vec<i64> = filtered.iter().map(|t| t.id).collect();
    assert_eq!(ids, vec![3, 2, 1]);
    assert!(
        template_rules::default_template_for(
            &templates,
            Some("G1"),
            template_rules::Role::Contract
        )
        .is_none()
    );
    assert_eq!(
        template_rules::default_template_for(
            &templates,
            Some("G2"),
            template_rules::Role::Contract
        )
        .map(|t| t.id),
        Some(4)
    );

    // T1.36 & T1.37 build_render_context + build_08a_context + T1.35 customer helpers
    let parse_date = |v: &Value| -> Option<NaiveDate> {
        v.as_str()
            .map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap())
    };
    let parse_dec = |v: &Value| -> Option<Decimal> {
        match v {
            Value::Null => None,
            Value::String(s) => Some(s.parse().unwrap()),
            Value::Number(n) => Some(n.to_string().parse().unwrap()),
            _ => None,
        }
    };
    for (idx, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let c = &case["contract"];
        let cu = &case["customer"];
        let s = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        let units = cu["sub_units"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|u| render_context::UnitView {
                name: s(&u["name"]),
                is_active: u["is_active"].as_bool().unwrap_or(false),
                include_in_billing: u["include_in_billing"].as_bool().unwrap_or(false),
                unit_type: s(&u["unit_type"]),
            })
            .collect();
        let cust = render_context::CustomerView {
            legal_name: s(&cu["legal_name"]),
            short_name: s(&cu["short_name"]),
            code: s(&cu["code"]),
            address: s(&cu["address"]),
            ward: s(&cu["ward"]),
            province: s(&cu["province"]),
            representative_name: s(&cu["representative_name"]),
            representative_gender: s(&cu["representative_gender"]),
            representative_title: s(&cu["representative_title"]),
            tax_code: s(&cu["tax_code"]),
            bank_account: s(&cu["bank_account"]),
            bank_name: s(&cu["bank_name"]),
            beneficiary_name: s(&cu["beneficiary_name"]),
            budget_relation_code: s(&cu["budget_relation_code"]),
            phone: s(&cu["phone"]),
            email: s(&cu["email"]),
            contact_name: s(&cu["contact_name"]),
            establishment_decision: s(&cu["establishment_decision"]),
            establishment_date: parse_date(&cu["establishment_date"]),
            bill_main_station: cu["bill_main_station"].as_bool().unwrap_or(false),
            record_count: cu["record_count"].as_i64(),
            units,
        };
        if idx == 0 {
            assert_eq!(
                customer::full_address(&cust),
                "Ấp 1, xã Sông Xanh, Tỉnh Mẫu"
            );
            assert_eq!(customer::total_units(&cust), 2);
            assert_eq!(
                customer::unit_names_list(&cust),
                vec!["TYT Sông Xanh", "Điểm trạm Ấp 2"]
            );
        }
        let parent = c["parent"].as_object().map(|p| render_context::ParentView {
            contract_number: p["contract_number"].as_str().unwrap().into(),
            sign_date: parse_date(&p["sign_date"]),
        });
        let product = c["product"]
            .as_object()
            .map(|p| render_context::ProductView {
                name: p["name"].as_str().unwrap().into(),
                code: p["code"].as_str().unwrap().into(),
            });
        let contract = render_context::ContractView {
            contract_number: c["contract_number"].as_str().unwrap().into(),
            doc_kind: c["doc_kind"].as_str().unwrap().into(),
            status: lifecycle::ContractStatus::Draft,
            party_b_representative: c["party_b_representative"].as_str().map(str::to_owned),
            party_b_authorization_number: c["party_b_authorization_number"]
                .as_str()
                .map(str::to_owned),
            party_b_authorization_date: c["party_b_authorization_date"].as_str().map(str::to_owned),
            total_pre_vat: parse_dec(&c["total_pre_vat"]),
            total_vat: parse_dec(&c["total_vat"]),
            payment_cycle_months: c["payment_cycle_months"].as_i64(),
            sign_location: c["sign_location"].as_str().map(str::to_owned),
            sign_date: parse_date(&c["sign_date"]),
            start_date: parse_date(&c["start_date"]),
            end_date: parse_date(&c["end_date"]),
            month_count: c["month_count"].as_i64(),
            unit_count: c["unit_count"].as_i64(),
            quantity: parse_dec(&c["quantity"]),
            unit_price_pre_vat: parse_dec(&c["unit_price_pre_vat"]),
            vat_rate: parse_dec(&c["vat_rate"]),
            unit_price_vat: parse_dec(&c["unit_price_vat"]),
            total_amount: parse_dec(&c["total_amount"]),
            amount_text: c["amount_text"].as_str().map(str::to_owned),
            pricing_description: c["pricing_description"].as_str().map(str::to_owned),
            acceptance_date: parse_date(&c["acceptance_date"]),
            liquidation_date: parse_date(&c["liquidation_date"]),
            contract_id: c["contract_id"].as_i64().unwrap(),
        };
        let party_b = render_context::PartyB::default();
        let ctx = render_context::build_render_context(
            &contract,
            &cust,
            parent.as_ref(),
            product.as_ref(),
            &party_b,
            None,
        );
        let expected_ctx: BTreeMap<String, String> = case["context"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
            .collect();
        assert_eq!(ctx, expected_ctx, "case {}", case["name"]);

        if let Some(expected_08a) = case["context_08a"].as_object() {
            let ctx_08a = render_context::build_08a_context(ctx, &cust, product.as_ref());
            let expected_08a: BTreeMap<String, String> = expected_08a
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                .collect();
            assert_eq!(ctx_08a, expected_08a, "08a case {}", case["name"]);
        }
    }

    // T1.39 lists_from_lines
    let lists_case = &fixture["lists_from_lines"];
    let lines: Vec<pricing::Line> = lists_case["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| pricing::Line {
            description: l["description"].as_str().unwrap().into(),
            unit_label: l["unit_label"].as_str().unwrap_or_default().into(),
            quantity: parse_dec(&l["quantity"]).unwrap(),
            period_count: parse_dec(&l["period_count"]).unwrap(),
            unit_price_pre_vat: parse_dec(&l["unit_price_pre_vat"]).unwrap(),
            vat_rate: parse_dec(&l["vat_rate"]).unwrap(),
            amount_pre_vat: parse_dec(&l["amount_pre_vat"]).unwrap(),
            vat_amount: parse_dec(&l["vat_amount"]).unwrap(),
            amount_total: parse_dec(&l["amount_total"]).unwrap(),
            product_id: l["product_id"].as_i64(),
            note: l["note"].as_str().unwrap_or_default().into(),
        })
        .collect();
    let features: Vec<Vec<render_context::Feature>> = lists_case["feature_lists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| {
            group
                .as_array()
                .unwrap()
                .iter()
                .map(|f| render_context::Feature {
                    name: f["name"].as_str().unwrap().into(),
                    description: f
                        .get("description")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                })
                .collect()
        })
        .collect();
    let actual_lists = render_context::lists_from_lines(&lines, &features);
    let expected_lists: BTreeMap<String, Vec<BTreeMap<String, String>>> = lists_case["output"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, rows)| {
            let parsed_rows = rows
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    r.as_object()
                        .unwrap()
                        .iter()
                        .map(|(rk, rv)| (rk.clone(), rv.as_str().unwrap().to_owned()))
                        .collect()
                })
                .collect();
            (k.clone(), parsed_rows)
        })
        .collect();
    assert_eq!(actual_lists, expected_lists);
}
