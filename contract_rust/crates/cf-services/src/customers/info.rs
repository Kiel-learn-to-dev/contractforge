use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    path::Path,
};

use cf_db::{
    model::{Contract, Customer, CustomerDocument, CustomerUnit},
    store::Store,
};
use chrono::NaiveDateTime;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{Ctx, ServiceError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerInfoExport {
    pub file_name: String,
    pub content: Vec<u8>,
}

pub fn build_info_md(
    customer: &Customer,
    units: &[CustomerUnit],
    contracts: &[Contract],
    products: &BTreeMap<i64, String>,
    now_local: NaiveDateTime,
) -> String {
    let value = |item: Option<&str>| item.unwrap_or_default().trim().to_owned();
    let name = customer
        .short_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or(&customer.legal_name);
    let full_address = [
        customer.address.as_deref(),
        customer.ward.as_deref(),
        customer.province.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(", ");
    let gender = match customer.representative_gender.as_deref().unwrap_or("") {
        "Nam" => "Ông",
        "Nữ" => "Bà",
        _ => "",
    };
    let unit_count = usize::from(customer.bill_main_station)
        + units
            .iter()
            .filter(|unit| unit.is_active && unit.include_in_billing)
            .count();
    let mut lines = vec![
        format!("# {name} — Thông tin khách hàng"),
        format!("*Xuất lúc: {}*", now_local.format("%d/%m/%Y %H:%M")),
        String::new(),
        "---".into(),
        String::new(),
        "## Thông tin cơ bản".into(),
        format!("- **Mã KH:** {}", value(Some(&customer.code))),
        format!("- **Tên pháp lý:** {}", value(Some(&customer.legal_name))),
        format!("- **Tên ngắn:** {}", value(customer.short_name.as_deref())),
        format!(
            "- **Loại đơn vị:** {}",
            value(Some(&customer.customer_type))
        ),
        String::new(),
        "## Địa chỉ".into(),
        format!("- **Địa chỉ:** {}", value(customer.address.as_deref())),
        format!("- **Xã / Phường:** {}", value(customer.ward.as_deref())),
        format!("- **Tỉnh / TP:** {}", value(customer.province.as_deref())),
        format!("- **Địa chỉ đầy đủ:** {full_address}"),
        String::new(),
        "## Pháp lý & Mã số".into(),
        format!("- **Mã số thuế:** {}", value(customer.tax_code.as_deref())),
        format!(
            "- **Mã QHNS:** {}",
            value(customer.budget_relation_code.as_deref())
        ),
        format!(
            "- **Mã cơ sở KCB BHYT:** {}",
            value(customer.kcb_code.as_deref())
        ),
        format!(
            "- **Số QĐ thành lập:** {}",
            value(customer.establishment_decision.as_deref())
        ),
        format!(
            "- **Ngày thành lập:** {}",
            customer
                .establishment_date
                .map(|date| date.format("%d/%m/%Y").to_string())
                .unwrap_or_default()
        ),
        format!(
            "- **Đại diện pháp lý:** {}",
            value(customer.representative_name.as_deref())
        ),
        format!("- **Giới tính đại diện:** {gender}"),
        format!(
            "- **Chức vụ:** {}",
            value(customer.representative_title.as_deref())
        ),
        String::new(),
        "## Ngân hàng".into(),
        format!(
            "- **Số tài khoản:** {}",
            value(customer.bank_account.as_deref())
        ),
        format!("- **Ngân hàng:** {}", value(customer.bank_name.as_deref())),
        format!(
            "- **Tên thụ hưởng:** {}",
            value(customer.beneficiary_name.as_deref())
        ),
        String::new(),
        "## Liên hệ".into(),
        format!(
            "- **Đầu mối liên hệ:** {}",
            value(customer.contact_name.as_deref())
        ),
        format!("- **Điện thoại:** {}", value(customer.phone.as_deref())),
        format!("- **Email:** {}", value(customer.email.as_deref())),
        String::new(),
        "## Cơ sở tính phí".into(),
        format!("- **Tổng số cơ sở:** {unit_count}"),
    ];
    if customer.bill_main_station {
        lines.push(format!(
            "  - {} *(trạm chính)*",
            value(
                customer
                    .short_name
                    .as_deref()
                    .or(Some(&customer.legal_name))
            )
        ));
    }
    for unit in units
        .iter()
        .filter(|unit| unit.is_active && unit.include_in_billing)
    {
        lines.push(format!(
            "  - {} *({})*",
            value(Some(&unit.name)),
            value(Some(&unit.unit_type))
        ));
    }
    if !contracts.is_empty() {
        lines.extend([
            String::new(),
            format!("## Hợp đồng ({} hợp đồng)", contracts.len()),
        ]);
        let mut sorted = contracts.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| b.sign_date.cmp(&a.sign_date).then_with(|| b.id.cmp(&a.id)));
        for contract in sorted {
            let product = contract
                .product_id
                .and_then(|id| products.get(&id))
                .map(String::as_str)
                .unwrap_or("");
            let product_part = if product.is_empty() {
                String::new()
            } else {
                format!(" · {product}")
            };
            let sign = contract
                .sign_date
                .map(|date| date.format("%d/%m/%Y").to_string())
                .unwrap_or_else(|| "—".into());
            let end = contract
                .end_date
                .map(|date| date.format("%d/%m/%Y").to_string())
                .unwrap_or_else(|| "—".into());
            lines.push(format!(
                "- **{}**{} · {} · {} → {}",
                value(Some(&contract.contract_number)),
                product_part,
                cf_core::lifecycle::status_label(contract.status.as_str()),
                sign,
                end
            ));
        }
    }
    if let Some(notes) = customer.notes.as_deref().filter(|notes| !notes.is_empty()) {
        lines.extend([String::new(), "## Ghi chú".into(), value(Some(notes))]);
    }
    lines.join("\n") + "\n"
}

pub fn export_info(
    store: &Store,
    customer_id: i64,
    now_local: NaiveDateTime,
) -> Result<CustomerInfoExport, ServiceError> {
    let customer = store
        .customers
        .get(&customer_id)
        .filter(|customer| !customer.is_deleted)
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy khách hàng".into()))?;
    let units = store
        .units
        .values()
        .filter(|unit| unit.customer_id == customer_id)
        .cloned()
        .collect::<Vec<_>>();
    let contracts = store
        .contracts
        .values()
        .filter(|contract| contract.customer_id == customer_id)
        .cloned()
        .collect::<Vec<_>>();
    let products = store
        .products
        .iter()
        .map(|(id, product)| {
            (
                *id,
                if product.name.is_empty() {
                    product.code.clone()
                } else {
                    product.name.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let name = customer
        .short_name
        .as_deref()
        .unwrap_or(&customer.legal_name);
    let file_name = format!("{}_{}.md", customer.code, name)
        .replace(' ', "_")
        .replace('/', "-");
    let content = build_info_md(customer, &units, &contracts, &products, now_local).into_bytes();
    Ok(CustomerInfoExport { file_name, content })
}

pub fn download_bundle(ctx: &Ctx, customer_id: i64) -> Result<CustomerInfoExport, ServiceError> {
    let customer = ctx
        .store()
        .customers
        .get(&customer_id)
        .filter(|customer| !customer.is_deleted)
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy khách hàng".into()))?;
    let info = export_info(ctx.store(), customer_id, ctx.clock().now_local())?;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("thong_tin.md", options).map_err(zip_error)?;
    zip.write_all(&info.content)?;
    let mut docs = ctx
        .store()
        .documents
        .values()
        .filter(|doc| doc.customer_id == customer_id)
        .cloned()
        .collect::<Vec<CustomerDocument>>();
    docs.sort_by(|a, b| a.doc_type.cmp(&b.doc_type).then_with(|| a.id.cmp(&b.id)));
    let mut other_index = 0usize;
    for doc in docs {
        let path = ctx.paths().resolve_stored(&doc.file_path);
        if !path.is_file() {
            continue;
        }
        let extension = Path::new(&doc.file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| format!(".{}", ext.to_lowercase()))
            .unwrap_or_default();
        let fixed = match doc.doc_type.as_str() {
            "cccd_front" => Some("CCCD_mat_truoc"),
            "cccd_back" => Some("CCCD_mat_sau"),
            "establishment" => Some("Quyet_dinh_thanh_lap"),
            _ => None,
        };
        let name = if let Some(prefix) = fixed {
            format!("{prefix}{extension}")
        } else {
            other_index += 1;
            let label = doc.label.as_deref().unwrap_or("Giay_to_khac");
            let safe = label
                .chars()
                .map(|ch| if "\\/:*?\"<>|".contains(ch) { '_' } else { ch })
                .collect::<String>()
                .trim()
                .to_owned();
            format!(
                "{other_index:02}_{}{extension}",
                if safe.is_empty() {
                    "Giay_to_khac"
                } else {
                    &safe
                }
            )
        };
        zip.start_file(name, options).map_err(zip_error)?;
        let bytes = fs::read(path)?;
        zip.write_all(&bytes)?;
    }
    let bytes = zip.finish().map_err(zip_error)?.into_inner();
    let name = customer
        .short_name
        .as_deref()
        .unwrap_or(&customer.legal_name);
    let file_name = format!("{}_{}.zip", customer.code, name)
        .replace(' ', "_")
        .replace('/', "-");
    Ok(CustomerInfoExport {
        file_name,
        content: bytes,
    })
}

fn zip_error(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Invalid(error.to_string())
}
