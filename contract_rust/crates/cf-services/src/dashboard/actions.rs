use cf_core::lifecycle::ACTIVE_LIFECYCLE;
use cf_db::store::Store;
use chrono::{Duration, NaiveDate, NaiveDateTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenTarget {
    Expiring { days: u16 },
    ContractsByStatus { status: String },
    NewTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionItem {
    pub level: &'static str,
    pub icon: &'static str,
    pub message: String,
    pub target: ScreenTarget,
    pub count: usize,
}

/// Port của dashboard_service.py::get_action_items; thứ tự và nội dung bám Python.
pub fn get_action_items(
    store: &Store,
    today: NaiveDate,
    now_utc: NaiveDateTime,
) -> Vec<ActionItem> {
    let mut items = Vec::new();
    let critical = store
        .contracts
        .values()
        .filter(|c| {
            ACTIVE_LIFECYCLE.contains(&c.status)
                && c.end_date
                    .is_some_and(|end| end > today && end <= today + Duration::days(7))
                && !excluded_from_expiry(store, c.id)
        })
        .count();
    if critical > 0 {
        items.push(ActionItem {
            level: "high",
            icon: "bi-alarm-fill",
            message: format!("{critical} hợp đồng hết hạn trong 7 ngày"),
            target: ScreenTarget::Expiring { days: 7 },
            count: critical,
        });
    }

    let draft_no_file = store
        .contracts
        .values()
        .filter(|c| c.status.as_str() == "Draft" && c.output_file_path.is_none())
        .count();
    if draft_no_file > 0 {
        items.push(ActionItem {
            level: "medium",
            icon: "bi-file-earmark-x",
            message: format!("{draft_no_file} hợp đồng nháp chưa được sinh file"),
            target: ScreenTarget::ContractsByStatus {
                status: "Draft".into(),
            },
            count: draft_no_file,
        });
    }

    let generated_stale = store
        .contracts
        .values()
        .filter(|c| {
            c.status.as_str() == "Generated" && c.updated_at <= now_utc - Duration::days(14)
        })
        .count();
    if generated_stale > 0 {
        items.push(ActionItem {
            level: "medium",
            icon: "bi-clock",
            message: format!("{generated_stale} hợp đồng đã sinh file, chưa gửi KH (>14 ngày)"),
            target: ScreenTarget::ContractsByStatus {
                status: "Generated".into(),
            },
            count: generated_stale,
        });
    }

    let signed_stale = store
        .contracts
        .values()
        .filter(|c| {
            c.status.as_str() == "Signed"
                && c.sign_date
                    .is_some_and(|date| date <= today - Duration::days(7))
        })
        .count();
    if signed_stale > 0 {
        items.push(ActionItem {
            level: "medium",
            icon: "bi-pen",
            message: format!("{signed_stale} hợp đồng đã ký, chưa chuyển hiệu lực (>7 ngày)"),
            target: ScreenTarget::ContractsByStatus {
                status: "Signed".into(),
            },
            count: signed_stale,
        });
    }

    if !store.templates.values().any(|template| template.is_active) {
        items.push(ActionItem {
            level: "low",
            icon: "bi-file-earmark-word",
            message: "Chưa có mẫu hợp đồng nào đang hoạt động".into(),
            target: ScreenTarget::NewTemplate,
            count: 0,
        });
    }
    items
}

fn excluded_from_expiry(store: &Store, contract_id: i64) -> bool {
    store.contracts.values().any(|other| {
        cf_core::lifecycle::RENEWAL_COUNTS.contains(&other.status)
            && ((other.parent_contract_id == Some(contract_id)
                && other.doc_kind == "addendum_renewal")
                || other.renewed_from_id == Some(contract_id))
    }) || store
        .contracts
        .get(&contract_id)
        .is_some_and(|c| c.doc_kind == "addendum_addon")
}
