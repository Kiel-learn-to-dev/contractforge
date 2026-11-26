use cf_core::render_context::{self, ParentView, ProductView};
use cf_db::store::Store;
use cf_docx::{RenderData, Template};

use crate::{ServiceError, settings};

/// Port của app/services/form08a_service.py::render_form_08a.
pub fn render_form_08a(store: &Store, contract_id: i64) -> Result<(Vec<u8>, String), ServiceError> {
    let contract = store.contracts.get(&contract_id).ok_or_else(|| {
        ServiceError::NotFound(format!("Không tìm thấy hợp đồng id={contract_id}"))
    })?;
    let customer = store.customers.get(&contract.customer_id).ok_or_else(|| {
        ServiceError::NotFound(format!(
            "Khách hàng id={} không tồn tại.",
            contract.customer_id
        ))
    })?;
    let product = contract.product_id.and_then(|id| store.products.get(&id));
    let parent = contract
        .parent_contract_id
        .and_then(|id| store.contracts.get(&id));
    let units = store
        .units
        .values()
        .filter(|unit| unit.customer_id == customer.id)
        .cloned()
        .collect::<Vec<_>>();
    let customer_view = customer.to_view_with_units(&units);
    let product_view = product.map(|product| ProductView {
        name: product.name.clone(),
        code: product.code.clone(),
    });
    let parent_view = parent.map(|parent| ParentView {
        contract_number: parent.contract_number.clone(),
        sign_date: parent.sign_date,
    });
    let mut context = render_context::build_render_context(
        &contract.to_view(),
        &customer_view,
        parent_view.as_ref(),
        product_view.as_ref(),
        &settings::get_party_b_settings(store),
        Some(&settings::addendum_format(store)),
    );
    context = render_context::build_08a_context(context, &customer_view, product_view.as_ref());

    let bytes = crate::assets::default_template_bytes("template_08A.docx")
        .expect("the embedded 08A sample template is available");
    let template = Template::compile(&bytes)?;
    let rendered = template
        .render(&RenderData {
            values: context,
            ..RenderData::default()
        })
        .map_err(|error| ServiceError::Invalid(format!("Lỗi khi render biểu 08A: {error}")))?;

    let contract_number = if contract.contract_number.is_empty() {
        contract_id.to_string()
    } else {
        contract.contract_number.clone()
    };
    let customer_code = if customer.code.is_empty() {
        "KH"
    } else {
        &customer.code
    };
    let file_name = format!(
        "08A_{}_{}.docx",
        cf_core::names::sanitize_for_filename(customer_code, 15),
        cf_core::names::sanitize_for_filename(&contract_number, 30),
    );
    Ok((rendered, file_name))
}
