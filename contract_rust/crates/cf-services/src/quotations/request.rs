//! Yêu cầu báo giá không tiết lộ dữ liệu giá.

use std::collections::BTreeSet;

use cf_core::{
    pricing::normalize_quantity,
    template_rules::{self, Role, TemplateInfo},
};
use cf_db::{model::ContractTemplate, store::Store};
use rust_decimal::Decimal;

use crate::ServiceError;

pub const ADDRESSEE_BLANK: &str = "……………………………………………";
pub const MAX_MONTHS: i64 = 120;

const SAMPLE_REQUEST_TEMPLATES: &[(&str, &str, &str, &str, &[u8])] = &[
    (
        "sample_request_letter.docx",
        "SAMPLE-RFQ-LETTER",
        "Thư mời báo giá (mẫu)",
        "Thư mời chính thức: quốc hiệu, kính gửi, bảng hạng mục và khối thông tin đối tác.",
        include_bytes!("../../../../assets/default_templates/sample_request_letter.docx"),
    ),
    (
        "sample_request_form.docx",
        "SAMPLE-RFQ-FORM",
        "Phiếu khảo sát giá gọn (mẫu)",
        "Một trang: bảng hạng mục và khối thông tin đối tác, hợp để in nhanh.",
        include_bytes!("../../../../assets/default_templates/sample_request_form.docx"),
    ),
    (
        "sample_request_detailed.docx",
        "SAMPLE-RFQ-DETAILED",
        "Yêu cầu báo giá kèm yêu cầu chức năng (mẫu)",
        "Thư mời kèm bảng yêu cầu chức năng, điều kiện báo giá và khối thông tin đối tác.",
        include_bytes!("../../../../assets/default_templates/sample_request_detailed.docx"),
    ),
];

/// Cài mẫu yêu cầu báo giá có sẵn, giữ nguyên các phiên bản đã có trong danh mục.
pub fn install_sample_templates(ctx: &mut crate::Ctx) -> Result<Vec<i64>, ServiceError> {
    use sha2::{Digest, Sha256};

    let mut created = Vec::new();
    for (filename, base_code, base_name, description, fallback_bytes) in SAMPLE_REQUEST_TEMPLATES {
        let bytes = crate::assets::default_template_bytes(filename)
            .unwrap_or_else(|| fallback_bytes.to_vec());
        let mut code = (*base_code).to_owned();
        let mut name = (*base_name).to_owned();
        if let Some(existing) = ctx
            .store()
            .templates
            .values()
            .find(|template| template.code == code)
        {
            let same = existing.file_path.as_deref().is_some_and(|raw| {
                std::fs::read(ctx.paths().resolve_stored(raw))
                    .is_ok_and(|existing_bytes| existing_bytes == bytes)
            });
            if same {
                continue;
            }
            let digest = Sha256::digest(&bytes);
            code = format!(
                "{base_code}-{:02X}{:02X}{:02X}{:02X}",
                digest[0], digest[1], digest[2], digest[3]
            );
            if ctx
                .store()
                .templates
                .values()
                .any(|template| template.code == code)
            {
                continue;
            }
            name = format!("{base_name} — bố cục mới");
        }
        created.push(crate::templates::create::create_template(
            ctx,
            crate::templates::create::TemplateDraft {
                code: &code,
                name: &name,
                contract_type: "Yêu cầu báo giá",
                file_bytes: &bytes,
                original_filename: filename,
                description,
                mode: "placeholder",
                product_group: "",
            },
        )?);
    }
    Ok(created)
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequestItem {
    pub name: String,
    pub unit_label: String,
    pub quantity: Decimal,
    pub period_count: Decimal,
    pub product_id: Option<i64>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RequestOptions {
    pub items: Vec<RequestRow>,
    pub features: Vec<FeatureRow>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RequestRow {
    pub item_no: String,
    pub item_name: String,
    pub item_unit: String,
    pub item_qty: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureRow {
    pub feature_no: String,
    pub feature_name: String,
    pub feature_desc: String,
}

#[derive(Debug, Clone)]
pub struct RequestContextInput<'a> {
    pub request_date: chrono::NaiveDate,
    pub deadline: Option<chrono::NaiveDate>,
    pub month_count: i64,
    pub addressee: &'a str,
    pub subject: &'a str,
    pub note: &'a str,
}

/// Port của ContractForge/app/services/quote_request.py::build_request_context.
pub fn build_request_context(
    customer: &cf_db::model::Customer,
    units: &[cf_db::model::CustomerUnit],
    input: RequestContextInput<'_>,
) -> std::collections::BTreeMap<String, String> {
    let RequestContextInput {
        request_date,
        deadline,
        month_count,
        addressee,
        subject,
        note,
    } = input;
    let view = customer.to_view_with_units(units);
    let address = [
        view.address.as_str(),
        view.ward.as_str(),
        view.province.as_str(),
    ]
    .into_iter()
    .filter(|v| !v.is_empty())
    .collect::<Vec<_>>()
    .join(", ");
    let mut ctx: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::from([
        ("PARTY_A_NAME".into(), customer.legal_name.trim().into()),
        ("PARTY_A_ADDRESS".into(), address),
        (
            "PARTY_A_TAX_CODE".into(),
            customer
                .tax_code
                .as_deref()
                .unwrap_or_default()
                .trim()
                .into(),
        ),
        (
            "PARTY_A_REPRESENTATIVE".into(),
            customer
                .representative_name
                .as_deref()
                .unwrap_or_default()
                .trim()
                .into(),
        ),
        (
            "PARTY_A_TITLE".into(),
            customer
                .representative_title
                .as_deref()
                .unwrap_or_default()
                .trim()
                .into(),
        ),
        (
            "PARTY_A_PHONE".into(),
            customer.phone.as_deref().unwrap_or_default().trim().into(),
        ),
        (
            "PARTY_A_EMAIL".into(),
            customer.email.as_deref().unwrap_or_default().trim().into(),
        ),
        (
            "PARTY_A_CONTACT_NAME".into(),
            customer
                .contact_name
                .as_deref()
                .unwrap_or_default()
                .trim()
                .into(),
        ),
        (
            "REQUEST_ISSUER_NAME".into(),
            customer.legal_name.trim().to_uppercase(),
        ),
        (
            "REQUEST_DATE".into(),
            request_date.format("%d/%m/%Y").to_string(),
        ),
        (
            "REQUEST_DATE_DAY".into(),
            request_date.format("%d").to_string(),
        ),
        (
            "REQUEST_DATE_MONTH".into(),
            request_date.format("%m").to_string(),
        ),
        (
            "REQUEST_DATE_YEAR".into(),
            request_date.format("%Y").to_string(),
        ),
        (
            "REQUEST_DEADLINE".into(),
            deadline.map_or_else(String::new, |d| d.format("%d/%m/%Y").to_string()),
        ),
        (
            "REQUEST_PLACE".into(),
            customer
                .province
                .as_deref()
                .unwrap_or_default()
                .trim()
                .into(),
        ),
        (
            "REQUEST_ADDRESSEE".into(),
            if addressee.trim().is_empty() {
                ADDRESSEE_BLANK.into()
            } else {
                addressee.trim().into()
            },
        ),
        ("REQUEST_SUBJECT".into(), subject.trim().into()),
        ("REQUEST_NOTE".into(), note.trim().into()),
        (
            "UNIT_COUNT".into(),
            cf_core::customer::total_units(&view).to_string(),
        ),
        ("MONTH_COUNT".into(), month_count.to_string()),
        (
            "PRICING_UNITS".into(),
            cf_core::customer::unit_names_list(&view).join("; "),
        ),
    ]);
    ctx.retain(|key, _| {
        !["PRICE", "AMOUNT", "TOTAL", "VAT"]
            .iter()
            .any(|word| key.contains(word))
    });
    ctx
}

/// Port của ContractForge/app/services/quote_request.py::build_request_lists.
pub fn build_request_lists(
    items: &[RequestItem],
    features_by_item: &[Vec<cf_core::render_context::Feature>],
) -> RequestOptions {
    let items = items
        .iter()
        .enumerate()
        .map(|(i, v)| RequestRow {
            item_no: (i + 1).to_string(),
            item_name: v.name.clone(),
            item_unit: v.unit_label.clone(),
            item_qty: (v.quantity * v.period_count).normalize().to_string(),
        })
        .collect();
    let mut seen = BTreeSet::new();
    let mut features = Vec::new();
    for group in features_by_item {
        for f in group {
            let name = f.name.trim();
            if name.is_empty() || !seen.insert(name.to_owned()) {
                continue;
            }
            features.push(FeatureRow {
                feature_no: (features.len() + 1).to_string(),
                feature_name: name.into(),
                feature_desc: f.description.as_deref().unwrap_or_default().trim().into(),
            });
        }
    }
    RequestOptions { items, features }
}

/// Port của ContractForge/app/services/quote_request.py::resolve_request_template.
pub fn resolve_request_template(
    store: &Store,
    id: i64,
    group: Option<&str>,
) -> Result<ContractTemplate, ServiceError> {
    let template = store
        .templates
        .get(&id)
        .filter(|t| t.is_active)
        .ok_or_else(|| ServiceError::Invalid("Mẫu không tồn tại hoặc đã ngừng sử dụng.".into()))?;
    let info = TemplateInfo {
        id: template.id,
        name: template.name.clone(),
        contract_type: template.contract_type.clone(),
        product_group: template.product_group.clone(),
        is_active: template.is_active,
    };
    template_rules::check_template_for(&info, group, Role::QuoteRequest)?;
    if template.file_path.as_deref().is_none_or(str::is_empty) {
        return Err(ServiceError::Invalid(
            "Mẫu đã chọn chưa có file DOCX.".into(),
        ));
    }
    Ok(template.clone())
}

/// Liệt kê mẫu yêu cầu báo giá đang bật, đúng nhóm hoặc mẫu dùng chung.
pub fn list_request_templates(store: &Store, group: Option<&str>) -> Vec<ContractTemplate> {
    let mut templates = store
        .templates
        .values()
        .filter(|template| template.is_active)
        .filter(|template| template.contract_type == template_rules::QUOTE_REQUEST_TYPE)
        .filter(|template| {
            template_rules::normalize_group(template.product_group.as_deref()).is_none_or(
                |template_group| template_rules::normalize_group(group) == Some(template_group),
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    templates.sort_by_key(|template| {
        (
            template_rules::normalize_group(template.product_group.as_deref()).is_none(),
            template.name.to_lowercase(),
        )
    });
    templates
}

/// Dựng tên file theo `quote_request.py`.
pub fn request_filename(
    customer: &cf_db::model::Customer,
    request_date: chrono::NaiveDate,
) -> String {
    let slug = customer
        .short_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or(&customer.legal_name)
        .replace(' ', "_")
        .chars()
        .take(30)
        .collect::<String>();
    format!("YeuCauBaoGia_{slug}_{}.docx", request_date.format("%Y%m%d"))
}

/// Sinh DOCX yêu cầu báo giá; context và danh sách dòng không chứa dữ liệu giá.
pub fn render_request_docx(
    paths: &cf_db::paths::DataPaths,
    template: &ContractTemplate,
    context: &std::collections::BTreeMap<String, String>,
    lists: &RequestOptions,
) -> Result<Vec<u8>, ServiceError> {
    let raw_path = template
        .file_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .ok_or_else(|| ServiceError::Invalid("Mẫu đã chọn chưa có file DOCX.".into()))?;
    let bytes = std::fs::read(paths.resolve_stored(raw_path))?;
    let compiled = cf_docx::Template::compile(&bytes)?;
    let mut values = context.clone();
    let values_by_canonical = cf_core::render_context::CANONICAL_MAP
        .iter()
        .filter_map(|(placeholder, canonical)| {
            context
                .get(*placeholder)
                .map(|value| ((*canonical).to_owned(), value.clone()))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for (placeholder, canonical) in &template.field_mapping {
        if let Some(value) = values_by_canonical.get(canonical) {
            values.insert(placeholder.clone(), value.clone());
        }
    }
    let item_rows = lists
        .items
        .iter()
        .map(|row| {
            std::collections::BTreeMap::from([
                ("ITEM_NO".into(), row.item_no.clone()),
                ("ITEM_NAME".into(), row.item_name.clone()),
                ("ITEM_UNIT".into(), row.item_unit.clone()),
                ("ITEM_QTY".into(), row.item_qty.clone()),
            ])
        })
        .collect();
    let feature_rows = lists
        .features
        .iter()
        .map(|row| {
            std::collections::BTreeMap::from([
                ("FEATURE_NO".into(), row.feature_no.clone()),
                ("FEATURE_NAME".into(), row.feature_name.clone()),
                ("FEATURE_DESC".into(), row.feature_desc.clone()),
            ])
        })
        .collect();
    let data = cf_docx::RenderData {
        values,
        lists: Some(std::collections::BTreeMap::from([
            ("items".into(), item_rows),
            ("features".into(), feature_rows),
        ])),
        omit_empty_tables: BTreeSet::from(["features".into()]),
        legacy_party_a_rows: false,
    };
    Ok(compiled.render(&data)?)
}

/// Port của ContractForge/app/services/quote_request.py::resolve_request_items.
pub fn resolve_request_items(
    store: &Store,
    customer_id: i64,
    main_product_id: i64,
    month_count: i64,
    main_name: &str,
    main_qty: &str,
    addons: &[(i64, String, String)],
) -> Result<(Vec<RequestItem>, Vec<Vec<cf_core::render_context::Feature>>), ServiceError> {
    if !(1..=MAX_MONTHS).contains(&month_count) {
        return Err(ServiceError::Invalid(format!(
            "Số tháng phải từ 1 đến {MAX_MONTHS}."
        )));
    }
    let customer = store
        .customers
        .get(&customer_id)
        .ok_or_else(|| ServiceError::Invalid("Khách hàng không tồn tại.".into()))?;
    let main = store
        .products
        .get(&main_product_id)
        .filter(|p| p.is_active && p.kind == "main")
        .ok_or_else(|| {
            ServiceError::Invalid("Sản phẩm chính không tồn tại hoặc đã ngừng sử dụng.".into())
        })?;
    let mut chosen = vec![(main, main_name, main_qty)];
    let mut seen = BTreeSet::from([main.id]);
    for (id, name, qty) in addons {
        let p = store
            .products
            .get(id)
            .filter(|p| p.is_active)
            .ok_or_else(|| {
                ServiceError::Invalid("Gói bổ sung không tồn tại hoặc đã ngừng sử dụng.".into())
            })?;
        if !seen.insert(*id) {
            continue;
        }
        if p.kind == "main" {
            return Err(ServiceError::Invalid(format!(
                "'{}' là gói chính, không thêm làm gói bổ sung.",
                p.name
            )));
        }
        if p.group_code.is_none() || p.group_code != main.group_code {
            return Err(ServiceError::Invalid(format!(
                "Gói bổ sung '{}' phải cùng nhóm với gói chính.",
                p.name
            )));
        }
        chosen.push((p, name.as_str(), qty.as_str()));
    }
    let units = store
        .units
        .values()
        .filter(|u| u.customer_id == customer_id)
        .cloned()
        .collect::<Vec<_>>();
    let view = customer.to_view_with_units(&units);
    let count = cf_core::customer::total_units(&view);
    let mut items = Vec::new();
    let mut features = Vec::new();
    for (p, name, raw) in chosen {
        let text = if raw.trim().is_empty() {
            match p.quantity_source.as_str() {
                "units" => count.max(1).to_string(),
                "records" => view
                    .record_count
                    .filter(|n| *n != 0)
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                _ => "1".into(),
            }
        } else {
            raw.trim().to_owned()
        };
        if text.is_empty() {
            return Err(ServiceError::Invalid(format!(
                "Gói '{}' cần nhập số lượng.",
                p.name
            )));
        }
        let d = text.replace(',', ".").parse::<Decimal>().map_err(|_| {
            ServiceError::Invalid(format!("Số lượng của '{}' không hợp lệ.", p.name))
        })?;
        let qty = normalize_quantity(
            &d.to_string(),
            p.min_quantity.map(Decimal::from),
            p.quantity_step.map(Decimal::from),
        )
        .map_err(|_| ServiceError::Invalid(format!("Số lượng của '{}' phải lớn hơn 0.", p.name)))?;
        let period = Decimal::from(month_count) / Decimal::from(p.price_period_months.max(1));
        items.push(RequestItem {
            name: if name.trim().is_empty() {
                p.name.clone()
            } else {
                name.trim().into()
            },
            unit_label: p.unit_label.clone().unwrap_or_default().trim().into(),
            quantity: qty,
            period_count: period,
            product_id: Some(p.id),
        });
        features.push(p.feature_list.clone());
    }
    Ok((items, features))
}

/// Port của ContractForge/app/services/quote_request.py::template_path.
pub fn template_path(paths: &cf_db::paths::DataPaths, raw: &str) -> std::path::PathBuf {
    paths.resolve_stored(raw)
}
#[allow(dead_code)]
fn _catalog_import_guard(_: &cf_db::model::Product) {
    let _ = normalize_quantity("1", None, None);
}
