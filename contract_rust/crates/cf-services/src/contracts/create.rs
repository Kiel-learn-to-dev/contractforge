//! Tạo bản nháp hợp đồng.

use std::collections::BTreeMap;

use cf_core::{
    dates::{compute_end_date, months_between},
    lifecycle::ContractStatus,
    pricing::{self, ItemInput, LegacyAmounts, Line},
};
use cf_db::{
    change::{Change, ChangeValue},
    model::{Contract, ContractItem},
};
use chrono::NaiveDate;
use rusqlite::types::Value;
use rust_decimal::Decimal;

use crate::{
    Ctx, ServiceError,
    events::{self, ChangeRef},
};

#[derive(Debug, Clone, Default)]
pub struct ContractForm {
    pub contract_number: String,
    pub customer_id: i64,
    pub product_id: Option<i64>,
    pub template_id: Option<i64>,
    pub sign_date: Option<NaiveDate>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub month_count: Option<i64>,
    pub sign_location: Option<String>,
    pub unit_count: Option<i64>,
    pub unit_price_pre_vat: Option<Decimal>,
    pub vat_rate: Option<Decimal>,
    pub unit_price_vat: Option<Decimal>,
    pub quantity: Option<Decimal>,
    pub total_amount: Option<Decimal>,
    pub amount_text: Option<String>,
    pub pricing_description: Option<String>,
    pub party_b_representative: Option<String>,
    pub party_b_authorization_number: Option<String>,
    pub party_b_authorization_date: Option<String>,
    pub acceptance_date: Option<NaiveDate>,
    pub liquidation_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub selected_unit_names: Vec<String>,
    pub items: Vec<ItemInput>,
    pub payment_mode: Option<String>,
    pub payment_cycle_months: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub struct CreateOptions<'a> {
    pub require_main: bool,
    pub template_role: &'a str,
    pub template_group: Option<&'a str>,
    pub parent_contract_id: Option<i64>,
    pub parent_event: Option<(&'a str, &'a str, &'a str)>,
    pub parent_event_contract_id: Option<i64>,
    pub renewed_from_id: Option<i64>,
    pub doc_kind: &'a str,
    pub created_event: Option<(&'a str, &'a str, &'a str)>,
    pub child_event: Option<(&'a str, &'a str, &'a str)>,
}

impl Default for CreateOptions<'_> {
    fn default() -> Self {
        Self {
            require_main: true,
            template_role: "contract",
            template_group: None,
            parent_contract_id: None,
            parent_event: None,
            parent_event_contract_id: None,
            renewed_from_id: None,
            doc_kind: "contract",
            created_event: None,
            child_event: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedContract {
    pub contract: Contract,
    pub lines: Vec<Line>,
}

/// Port của app/services/contract_service.py::create_draft.
pub fn create_draft(
    ctx: &mut Ctx,
    form: ContractForm,
    opts: CreateOptions<'_>,
) -> Result<i64, ServiceError> {
    let now = ctx.clock().now_utc();
    let prepared = prepare_contract(ctx, form, opts, now, true)?;
    let mut changes = insert_contract_changes(&prepared.contract, &prepared.lines);
    let (kind, description, actor) =
        opts.created_event
            .unwrap_or(("created", "Tạo bản nháp hợp đồng", "system"));
    changes.push(events::event(
        ChangeRef::New(0),
        kind,
        description,
        actor,
        now,
    ));
    if let Some((kind, description, actor)) = opts.child_event {
        changes.push(events::event(
            ChangeRef::New(0),
            kind,
            description,
            actor,
            now,
        ));
    }
    if let Some((kind, description, actor)) = opts.parent_event
        && let Some(parent_id) = opts.parent_event_contract_id.or(opts.parent_contract_id)
    {
        changes.push(events::event(
            ChangeRef::Existing(parent_id),
            kind,
            description,
            actor,
            now,
        ));
    }
    let result = ctx.store_mut().apply(changes)?;
    result
        .inserted_ids
        .first()
        .copied()
        .ok_or_else(|| ServiceError::Invalid("Không tạo được bản nháp hợp đồng.".into()))
}

/// Dựng hợp đồng và các dòng mà chưa ghi DB, để luồng batch render trước khi commit.
pub fn prepare_contract(
    ctx: &mut Ctx,
    form: ContractForm,
    opts: CreateOptions<'_>,
    now: chrono::NaiveDateTime,
    require_active_template: bool,
) -> Result<PreparedContract, ServiceError> {
    let number = form.contract_number.trim();
    if number.is_empty() {
        return Err(ServiceError::Invalid(
            "Số hợp đồng không được để trống.".into(),
        ));
    }
    if form.customer_id <= 0 || !ctx.store().customers.contains_key(&form.customer_id) {
        return Err(ServiceError::Invalid("Phải chọn khách hàng.".into()));
    }
    if ctx
        .store()
        .contracts
        .values()
        .any(|c| c.contract_number == number)
    {
        return Err(ServiceError::Invalid(format!(
            "Số hợp đồng '{number}' đã tồn tại."
        )));
    }
    let template = match form.template_id {
        Some(id) => {
            let value = ctx
                .store()
                .templates
                .get(&id)
                .ok_or_else(|| ServiceError::Invalid("Mẫu không tồn tại.".into()))?;
            if require_active_template && !value.is_active {
                return Err(ServiceError::Invalid(format!(
                    "Mẫu '{}' đang tắt.",
                    value.name
                )));
            }
            Some(value.clone())
        }
        None => None,
    };

    let sign_date = form.sign_date;
    let start_date = form.start_date;
    let month_count = form.month_count.or_else(|| {
        form.start_date
            .zip(form.end_date)
            .map(|(s, e)| i64::from(months_between(s, e)))
    });
    let end_date = form.end_date.or_else(|| {
        form.start_date.zip(month_count).and_then(|(s, m)| {
            i32::try_from(m)
                .ok()
                .map(|months| compute_end_date(s, months))
        })
    });
    let unit_count = form.unit_count.filter(|v| *v != 0).unwrap_or(1);
    let legacy: LegacyAmounts = pricing::legacy_amounts(
        Some(Decimal::from(unit_count)),
        month_count.map(Decimal::from),
        form.unit_price_pre_vat,
        Some(form.vat_rate.unwrap_or_default()),
        form.unit_price_vat,
        form.quantity,
        form.total_amount,
    );
    let lines: Option<Vec<Line>> = if form.items.is_empty() {
        None
    } else {
        let months = month_count
            .filter(|m| *m > 0)
            .ok_or_else(|| ServiceError::Invalid("Phải có số tháng để tính tiền.".into()))?;
        Some(pricing::build_lines(
            &form.items,
            |id| ctx.store().products.get(&id).map(|p| p.to_rule()),
            months,
            opts.require_main,
        )?)
    };
    let totals = lines
        .as_ref()
        .map(|ls| pricing::sum_lines(ls))
        .transpose()?;
    let mut contract = Contract {
        id: 0,
        contract_number: number.to_owned(),
        status: ContractStatus::Draft,
        customer_id: form.customer_id,
        product_id: lines
            .as_ref()
            .and_then(|ls| ls.first().and_then(|l| l.product_id))
            .or(form.product_id),
        template_id: form.template_id,
        sign_date,
        start_date,
        end_date,
        month_count,
        sign_location: clean(form.sign_location),
        unit_count: Some(unit_count),
        quantity: legacy.quantity,
        unit_price_pre_vat: legacy.unit_price_pre_vat,
        vat_rate: legacy.vat_rate,
        unit_price_vat: legacy.unit_price_vat,
        total_amount: legacy.total_amount,
        amount_text: clean(form.amount_text).or_else(|| {
            (lines.is_none()
                && legacy.total_amount.is_some_and(|v| !v.is_zero())
                && !legacy.amount_text.is_empty())
            .then_some(legacy.amount_text.clone())
        }),
        pricing_description: clean(form.pricing_description),
        party_b_representative: clean(form.party_b_representative),
        party_b_authorization_number: clean(form.party_b_authorization_number),
        party_b_authorization_date: clean(form.party_b_authorization_date),
        acceptance_date: form.acceptance_date,
        liquidation_date: form.liquidation_date,
        output_file_path: None,
        output_file_name: None,
        signed_pdf_path: None,
        signed_pdf_name: None,
        signed_pdf_uploaded_at: None,
        invoice_date: None,
        invoice_pdf_path: None,
        invoice_pdf_name: None,
        invoice_pdf_uploaded_at: None,
        payment_date: None,
        payment_slip_path: None,
        payment_slip_name: None,
        payment_slip_uploaded_at: None,
        selected_unit_names: form
            .selected_unit_names
            .into_iter()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
            .collect(),
        render_data: serde_json::Value::Null,
        notes: clean(form.notes),
        is_batch: false,
        batch_job_id: None,
        pricing_model: "legacy".into(),
        total_pre_vat: None,
        total_vat: None,
        payment_mode: None,
        payment_cycle_months: None,
        parent_contract_id: opts.parent_contract_id,
        renewed_from_id: opts.renewed_from_id,
        doc_kind: opts.doc_kind.into(),
        created_at: now,
        updated_at: now,
    };
    if let (Some(ls), Some(t)) = (&lines, &totals) {
        apply_lines(&mut contract, ls, t);
        contract.payment_mode = form
            .payment_mode
            .as_deref()
            .map(str::trim)
            .map(str::to_lowercase)
            .filter(|value| matches!(value.as_str(), "prepaid" | "postpaid"));
        contract.payment_cycle_months = form.payment_cycle_months;
    }
    if let Some(template) = template {
        let group = opts.template_group.or_else(|| {
            contract.product_id.and_then(|id| {
                ctx.store()
                    .products
                    .get(&id)
                    .and_then(|p| p.group_code.as_deref())
            })
        });
        let role = match opts.template_role {
            "addon" => cf_core::template_rules::Role::Addon,
            "renewal" => cf_core::template_rules::Role::Renewal,
            "attachment" => cf_core::template_rules::Role::Attachment,
            "quotation" => cf_core::template_rules::Role::Quotation,
            "quote_request" => cf_core::template_rules::Role::QuoteRequest,
            _ => cf_core::template_rules::Role::Contract,
        };
        cf_core::template_rules::check_template_for(
            &cf_core::template_rules::TemplateInfo {
                id: template.id,
                name: template.name.clone(),
                contract_type: template.contract_type.clone(),
                product_group: template.product_group.clone(),
                is_active: template.is_active,
            },
            group,
            role,
        )?;
    }

    Ok(PreparedContract {
        contract,
        lines: lines.unwrap_or_default(),
    })
}

/// Insert hợp đồng và các dòng trong cùng transaction. Hợp đồng luôn là Insert #0,
/// vì vậy FK của từng dòng dùng ChangeValue::Ref(0).
pub fn insert_contract_changes(contract: &Contract, lines: &[Line]) -> Vec<Change> {
    let contract_values = sql(contract.to_values());
    let mut changes = vec![Change::insert("contracts", contract_values)];
    for (idx, line) in lines.iter().enumerate() {
        let item = ContractItem {
            id: 0,
            contract_id: 0,
            product_id: line.product_id,
            sort_order: idx as i64,
            description: line.description.clone(),
            unit_label: Some(line.unit_label.clone()),
            quantity: line.quantity,
            period_count: line.period_count,
            unit_price_pre_vat: Some(line.unit_price_pre_vat),
            vat_rate: line.vat_rate,
            amount_pre_vat: line.amount_pre_vat,
            vat_amount: line.vat_amount,
            amount_total: line.amount_total,
            note: clean(Some(line.note.clone())),
        };
        let mut values = item
            .to_values()
            .into_iter()
            .filter(|(key, _)| key != "id")
            .map(|(key, value)| (key, ChangeValue::Sql(value)))
            .collect::<BTreeMap<_, _>>();
        values.insert("contract_id".into(), ChangeValue::Ref(0));
        changes.push(Change::insert("contract_items", values));
    }
    changes
}

/// Đặt các trường tiền của hợp đồng theo dòng đã tính; giữ số chữ và mô tả người dùng nhập.
pub fn apply_lines(contract: &mut Contract, lines: &[Line], totals: &pricing::Totals) {
    let Some(main) = lines.first() else {
        return;
    };
    contract.pricing_model = "items".into();
    contract.product_id = main.product_id.or(contract.product_id);
    contract.unit_count =
        Some(i64::try_from(cf_core::money::trunc_int(main.quantity)).unwrap_or(1));
    contract.unit_price_pre_vat = Some(main.unit_price_pre_vat);
    contract.vat_rate = Some(main.vat_rate);
    contract.unit_price_vat = Some(cf_core::pricing::unit_price_with_vat(
        main.unit_price_pre_vat,
        main.vat_rate,
    ));
    contract.quantity = Some(main.quantity * main.period_count);
    contract.total_amount = Some(totals.total);
    contract.total_pre_vat = Some(totals.pre_vat);
    contract.total_vat = Some(totals.vat);
    if contract
        .amount_text
        .as_deref()
        .is_none_or(str::trim_is_empty)
    {
        contract.amount_text = (!totals.amount_text.is_empty()).then(|| totals.amount_text.clone());
    }
    if contract
        .pricing_description
        .as_deref()
        .is_none_or(str::trim_is_empty)
    {
        contract.pricing_description = Some(pricing::describe(lines, totals));
    }
}

trait TrimIsEmpty {
    fn trim_is_empty(&self) -> bool;
}
impl TrimIsEmpty for str {
    fn trim_is_empty(&self) -> bool {
        self.trim().is_empty()
    }
}

fn clean(value: Option<String>) -> Option<String> {
    value.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}
fn sql(values: BTreeMap<String, Value>) -> BTreeMap<String, ChangeValue> {
    values
        .into_iter()
        .filter(|(k, _)| k != "id")
        .map(|(k, v)| (k, ChangeValue::Sql(v)))
        .collect()
}
