use cf_core::{names::make_contract_slug, numbering};
use cf_db::store::Store;

use crate::ServiceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facility {
    pub key: String,
    pub name: String,
    pub unit_type: String,
    pub checked: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacilityList {
    pub total_units: usize,
    pub unit_names: Vec<String>,
    pub facilities: Vec<Facility>,
}

pub fn suggest_number(
    store: &Store,
    customer_id: i64,
    product_id: Option<i64>,
    year: i32,
) -> Result<String, ServiceError> {
    let customer = store
        .customers
        .get(&customer_id)
        .ok_or_else(|| ServiceError::NotFound("Khách hàng không tồn tại".into()))?;
    let format = product_id
        .and_then(|id| store.products.get(&id))
        .and_then(|product| product.contract_number_format.as_deref())
        .filter(|format| !format.trim().is_empty())
        .unwrap_or(numbering::DEFAULT_FORMAT);
    let existing = store
        .indexes
        .contracts_by_customer
        .get(&customer_id)
        .into_iter()
        .flatten()
        .filter_map(|id| store.contracts.get(id))
        .map(|contract| contract.contract_number.as_str())
        .collect::<Vec<_>>();
    numbering::next_number(
        format,
        &make_contract_slug(&customer.legal_name),
        year,
        &existing,
    )
    .map_err(ServiceError::from)
}

pub fn customer_facilities(store: &Store, customer_id: Option<i64>) -> FacilityList {
    let Some(customer) = customer_id.and_then(|id| store.customers.get(&id)) else {
        return FacilityList {
            total_units: 1,
            unit_names: Vec::new(),
            facilities: Vec::new(),
        };
    };
    let mut facilities = Vec::new();
    if customer.bill_main_station {
        facilities.push(Facility {
            key: "main".into(),
            name: customer
                .short_name
                .as_deref()
                .filter(|name| !name.is_empty())
                .unwrap_or(&customer.legal_name)
                .to_owned(),
            unit_type: "Trạm chính".into(),
            checked: true,
        });
    }
    if let Some(ids) = store.indexes.units_by_customer.get(&customer.id) {
        for unit in ids
            .iter()
            .filter_map(|id| store.units.get(id))
            .filter(|unit| unit.is_active)
        {
            facilities.push(Facility {
                key: format!("u{}", unit.id),
                name: unit.name.clone(),
                unit_type: if unit.unit_type.trim().is_empty() {
                    "Điểm trạm".into()
                } else {
                    unit.unit_type.clone()
                },
                checked: unit.include_in_billing,
            });
        }
    }
    let unit_names = facilities
        .iter()
        .filter(|facility| facility.checked)
        .map(|facility| facility.name.clone())
        .collect::<Vec<_>>();
    FacilityList {
        total_units: unit_names.len().max(1),
        unit_names,
        facilities,
    }
}
