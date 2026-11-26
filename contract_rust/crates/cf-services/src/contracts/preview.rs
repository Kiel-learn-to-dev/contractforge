use cf_core::{
    error::CoreError,
    pricing::{self, ItemInput, Line},
};
use cf_db::store::Store;
use rust_decimal::Decimal;

use crate::ServiceError;

#[derive(Debug, Clone, PartialEq)]
pub struct PricePreview {
    pub lines: Vec<Line>,
    pub totals: pricing::Totals,
    pub description: String,
}

pub fn price_preview(
    store: &Store,
    items: &[ItemInput],
    month_count: i64,
    require_main: bool,
) -> Result<PricePreview, ServiceError> {
    if month_count < 1 {
        return Err(ServiceError::Invalid(
            "Phải có số tháng để tính tiền.".into(),
        ));
    }
    let lines = pricing::build_lines(
        items,
        |id| store.products.get(&id).map(|product| product.to_rule()),
        month_count,
        require_main,
    )
    .map_err(ServiceError::from)?;
    let totals = pricing::sum_lines(&lines).map_err(ServiceError::from)?;
    let description = pricing::describe(&lines, &totals);
    Ok(PricePreview {
        lines,
        totals,
        description,
    })
}

pub fn preview_total(preview: &PricePreview) -> Decimal {
    preview.totals.total
}

pub fn validate_month_count(months: i64) -> Result<(), ServiceError> {
    if months < 1 {
        Err(CoreError::Invalid("Phải có số tháng để tính tiền.".into()).into())
    } else {
        Ok(())
    }
}
