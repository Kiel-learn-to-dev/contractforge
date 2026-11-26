use cf_core::pricing::Line;
use cf_core::render_context::{self, ParentView, ProductView, TemplateMapping};
use cf_db::store::Store;
use cf_docx::RenderData;

use crate::{ServiceError, settings};

/// Port của app/services/docx_renderer.py::build_render_context.
pub fn render_data_for(
    store: &Store,
    contract_id: i64,
    template_id: i64,
) -> Result<RenderData, ServiceError> {
    render_data_for_with_legacy_rows(store, contract_id, template_id, None)
}

/// Builds a render context while reusing the template analysis already held by the caller.
/// Batch generation uses this entry point so a shared template is only compiled once.
pub fn render_data_for_with_legacy_rows(
    store: &Store,
    contract_id: i64,
    template_id: i64,
    legacy_party_a_rows: Option<bool>,
) -> Result<RenderData, ServiceError> {
    let contract = store
        .contracts
        .get(&contract_id)
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy hợp đồng.".into()))?;
    let customer = store
        .customers
        .get(&contract.customer_id)
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy khách hàng.".into()))?;
    let template = store
        .templates
        .get(&template_id)
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy mẫu.".into()))?;

    let product = contract.product_id.and_then(|id| store.products.get(&id));
    let parent = contract
        .parent_contract_id
        .and_then(|id| store.contracts.get(&id));
    let units = store
        .units
        .values()
        .filter(|unit| unit.customer_id == customer.id)
        .collect::<Vec<_>>();
    let customer_view =
        customer.to_view_with_units(&units.into_iter().cloned().collect::<Vec<_>>());
    let product_view = product.map(|value| ProductView {
        name: value.name.clone(),
        code: value.code.clone(),
    });
    let parent_view = parent.map(|value| ParentView {
        contract_number: value.contract_number.clone(),
        sign_date: value.sign_date,
    });
    let mut values = render_context::build_render_context(
        &contract.to_view(),
        &customer_view,
        parent_view.as_ref(),
        product_view.as_ref(),
        &settings::get_party_b_settings(store),
        Some(&settings::addendum_format(store)),
    );

    let lines = store
        .items
        .values()
        .filter(|item| item.contract_id == contract_id)
        .collect::<Vec<_>>();
    let mut lines = lines;
    lines.sort_by_key(|item| (item.sort_order, item.id));
    let core_lines = lines
        .iter()
        .map(|item| Line {
            description: item.description.clone(),
            unit_label: item.unit_label.clone().unwrap_or_default(),
            quantity: item.quantity,
            period_count: item.period_count,
            unit_price_pre_vat: item.unit_price_pre_vat.unwrap_or_default(),
            vat_rate: item.vat_rate,
            amount_pre_vat: item.amount_pre_vat,
            vat_amount: item.vat_amount,
            amount_total: item.amount_total,
            product_id: item.product_id,
            note: item.note.clone().unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    let feature_lists = lines
        .iter()
        .map(|item| {
            item.product_id
                .and_then(|id| store.products.get(&id))
                .map(|product| product.feature_list.clone())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let lists = render_context::lists_from_lines(&core_lines, &feature_lists);

    let mapping = TemplateMapping(template.field_mapping.clone());
    let has_08a_fields = template
        .detected_placeholders
        .iter()
        .filter_map(|placeholder| {
            render_context::CANONICAL_MAP
                .iter()
                .find(|(key, _)| key == placeholder)
                .map(|(_, field)| *field)
        })
        .chain(template.field_mapping.values().map(String::as_str))
        .any(|field| field.starts_with("form08a."));
    if has_08a_fields {
        values = render_context::build_08a_context(values, &customer_view, product_view.as_ref());
    }
    values = render_context::apply_template_mapping(&values, &mapping);

    let legacy_party_a_rows = match legacy_party_a_rows {
        Some(value) => value,
        None => {
            let path = template.file_path.as_deref().ok_or_else(|| {
                ServiceError::FileMissing("File mẫu không tồn tại trên đĩa.".into())
            })?;
            let compiled = cf_docx::Template::compile(&std::fs::read(path).map_err(|_| {
                ServiceError::FileMissing("File mẫu không tồn tại trên đĩa.".into())
            })?)?;
            compiled.analysis().optional_keys.is_empty()
        }
    };

    Ok(RenderData {
        values,
        lists: Some(lists),
        omit_empty_tables: Default::default(),
        legacy_party_a_rows,
    })
}
