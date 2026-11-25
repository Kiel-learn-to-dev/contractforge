//! Kiểu dữ liệu khách hàng dùng trong nghiệp vụ thuần.

use crate::render_context::CustomerView;

/// Port của app/models/customer.py::Customer.full_address.
pub fn full_address(customer: &CustomerView) -> String {
    [&customer.address, &customer.ward, &customer.province]
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(|part| part.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Port của app/models/customer.py::Customer.total_units.
pub fn total_units(customer: &CustomerView) -> usize {
    (if customer.bill_main_station { 1 } else { 0 })
        + customer
            .units
            .iter()
            .filter(|unit| unit.is_active && unit.include_in_billing)
            .count()
}

/// Port của app/models/customer.py::Customer.unit_names_list.
pub fn unit_names_list(customer: &CustomerView) -> Vec<String> {
    let mut names = Vec::new();
    if customer.bill_main_station {
        names.push(if customer.short_name.is_empty() {
            customer.legal_name.clone()
        } else {
            customer.short_name.clone()
        });
    }
    names.extend(
        customer
            .units
            .iter()
            .filter(|unit| unit.is_active && unit.include_in_billing)
            .map(|unit| unit.name.clone()),
    );
    names
}
