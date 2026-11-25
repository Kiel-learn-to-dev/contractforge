//! Quy tắc chọn mẫu.

use crate::error::CoreError;

pub const QUOTATION_TYPE: &str = "Báo giá";
pub const QUOTE_REQUEST_TYPE: &str = "Yêu cầu báo giá";
pub const ADDON_TYPE: &str = "Phụ lục bổ sung";
pub const RENEWAL_TYPE: &str = "Phụ lục gia hạn";
pub const LEGACY_APPENDIX_TYPE: &str = "Phụ lục";
pub const ATTACHMENT_TYPES: &[&str] = &["Nghiệm thu", "Bàn giao", "Thanh lý", "Khác"];
pub const PRESELECT_ATTACHMENT_TYPES: &[&str] = &["Bàn giao", "Nghiệm thu"];
pub const CONTRACT_TYPES: &[&str] = &[
    "Hợp đồng",
    ADDON_TYPE,
    RENEWAL_TYPE,
    "Nghiệm thu",
    "Bàn giao",
    "Thanh lý",
    QUOTATION_TYPE,
    QUOTE_REQUEST_TYPE,
    "Khác",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Contract,
    Addon,
    Renewal,
    Attachment,
    Quotation,
    QuoteRequest,
}

/// Port của app/services/template_service.py::has_role.
pub fn has_role(contract_type: &str, role: Role) -> bool {
    match role {
        Role::Contract => ![
            ADDON_TYPE,
            RENEWAL_TYPE,
            LEGACY_APPENDIX_TYPE,
            "Nghiệm thu",
            "Bàn giao",
            "Thanh lý",
            "Khác",
            QUOTATION_TYPE,
            QUOTE_REQUEST_TYPE,
        ]
        .contains(&contract_type),
        Role::Addon => [ADDON_TYPE, LEGACY_APPENDIX_TYPE].contains(&contract_type),
        Role::Renewal => [RENEWAL_TYPE, LEGACY_APPENDIX_TYPE].contains(&contract_type),
        Role::Attachment => ATTACHMENT_TYPES.contains(&contract_type),
        Role::Quotation => contract_type == QUOTATION_TYPE,
        Role::QuoteRequest => contract_type == QUOTE_REQUEST_TYPE,
    }
}

pub fn normalize_group(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Port của app/services/template_service.py::role_of_doc_kind.
pub fn role_of_doc_kind(kind: Option<&str>) -> Role {
    match kind.unwrap_or("contract") {
        "addendum_addon" => Role::Addon,
        "addendum_renewal" => Role::Renewal,
        _ => Role::Contract,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateInfo {
    pub id: i64,
    pub name: String,
    pub contract_type: String,
    pub product_group: Option<String>,
    pub is_active: bool,
}

/// Port của app/services/template_service.py::check_template_for.
pub fn check_template_for(
    template: &TemplateInfo,
    group: Option<&str>,
    role: Role,
) -> Result<(), CoreError> {
    if !has_role(&template.contract_type, role) {
        return Err(CoreError::Invalid(format!(
            "Mẫu '{}' loại '{}' không dùng được ở đây.",
            template.name, template.contract_type
        )));
    }
    let template_group = normalize_group(template.product_group.as_deref());
    let group = normalize_group(group);
    if let Some(template_group) =
        template_group.filter(|template_group| Some(template_group) != group.as_ref())
    {
        let where_group = group.as_deref().unwrap_or("chưa có nhóm");
        return Err(CoreError::Invalid(format!(
            "Mẫu '{}' thuộc nhóm sản phẩm '{}', không dùng cho nhóm '{}'.",
            template.name, template_group, where_group
        )));
    }
    Ok(())
}

pub fn preselected_attachment(template: &TemplateInfo) -> bool {
    normalize_group(template.product_group.as_deref()).is_some()
        && PRESELECT_ATTACHMENT_TYPES.contains(&template.contract_type.as_str())
}

/// Port của app/services/template_service.py::_template_kind.
pub fn template_kind(placeholders: &[String], heading: &str) -> &'static str {
    let fields: std::collections::BTreeSet<&str> =
        placeholders.iter().map(String::as_str).collect();
    let title = heading.to_uppercase();
    if title.contains("BIÊN BẢN") {
        if title.contains("BÀN GIAO") {
            return "Bàn giao";
        }
        if title.contains("NGHIỆM THU") {
            return "Nghiệm thu";
        }
    }
    if fields.iter().any(|field| field.starts_with("REQUEST_DATE")) {
        return QUOTE_REQUEST_TYPE;
    }
    if fields.iter().any(|field| field.starts_with("QUOTE_DATE")) {
        return QUOTATION_TYPE;
    }
    if fields
        .iter()
        .any(|field| field.starts_with("ACCEPTANCE_DATE"))
    {
        return "Nghiệm thu";
    }
    if fields
        .iter()
        .any(|field| field.starts_with("LIQUIDATION_DATE"))
    {
        return "Thanh lý";
    }
    if fields.contains("PARENT_CONTRACT_NUMBER") {
        if fields.contains("START_DATE") && fields.contains("END_DATE") {
            return RENEWAL_TYPE;
        }
        return ADDON_TYPE;
    }
    "Hợp đồng"
}

/// Port của app/services/template_service.py::templates_for.
pub fn templates_for<'a>(
    templates: &'a [TemplateInfo],
    group: Option<&str>,
    role: Role,
    active_only: bool,
) -> Vec<&'a TemplateInfo> {
    let group = normalize_group(group);
    let mut rows: Vec<_> = templates
        .iter()
        .filter(|template| {
            (!active_only || template.is_active)
                && has_role(&template.contract_type, role)
                && normalize_group(template.product_group.as_deref())
                    .is_none_or(|template_group| Some(template_group) == group)
        })
        .collect();
    rows.sort_by_key(|template| {
        (
            normalize_group(template.product_group.as_deref()).is_none(),
            template.name.to_lowercase(),
        )
    });
    rows
}

/// Port của app/services/template_service.py::default_template_for.
pub fn default_template_for<'a>(
    templates: &'a [TemplateInfo],
    group: Option<&str>,
    role: Role,
) -> Option<&'a TemplateInfo> {
    let group = normalize_group(group)?;
    let own: Vec<_> = templates_for(templates, Some(&group), role, true)
        .into_iter()
        .filter(|template| {
            normalize_group(template.product_group.as_deref()).as_ref() == Some(&group)
        })
        .collect();
    if own.len() == 1 { Some(own[0]) } else { None }
}
