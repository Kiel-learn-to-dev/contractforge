//! Vòng đời hợp đồng.

use crate::error::CoreError;
use chrono::NaiveDate;
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContractStatus {
    #[default]
    Draft,
    Generated,
    Sent,
    Signed,
    Invoiced,
    PaidActive,
    Active,
    ExpiringSoon,
    Expired,
    Terminated,
    Unknown(String),
}

impl ContractStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Draft => "Draft",
            Self::Generated => "Generated",
            Self::Sent => "Sent",
            Self::Signed => "Signed",
            Self::Invoiced => "Invoiced",
            Self::PaidActive => "PaidActive",
            Self::Active => "Active",
            Self::ExpiringSoon => "ExpiringSoon",
            Self::Expired => "Expired",
            Self::Terminated => "Terminated",
            Self::Unknown(value) => value,
        }
    }
    pub fn label(&self) -> &str {
        status_label(self.as_str())
    }
    pub fn color(&self) -> &'static str {
        status_color(self.as_str())
    }
}

impl fmt::Display for ContractStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl FromStr for ContractStatus {
    type Err = std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "Draft" => Self::Draft,
            "Generated" => Self::Generated,
            "Sent" => Self::Sent,
            "Signed" => Self::Signed,
            "Invoiced" => Self::Invoiced,
            "PaidActive" => Self::PaidActive,
            "Active" => Self::Active,
            "ExpiringSoon" => Self::ExpiringSoon,
            "Expired" => Self::Expired,
            "Terminated" => Self::Terminated,
            other => Self::Unknown(other.to_owned()),
        })
    }
}

pub const ACTIVE_LIFECYCLE: &[ContractStatus] = &[
    ContractStatus::Signed,
    ContractStatus::Invoiced,
    ContractStatus::PaidActive,
    ContractStatus::Active,
    ContractStatus::ExpiringSoon,
];
pub const RENEWAL_COUNTS: &[ContractStatus] = ACTIVE_LIFECYCLE;
pub const IN_SERVICE: &[ContractStatus] = &[
    ContractStatus::Invoiced,
    ContractStatus::PaidActive,
    ContractStatus::Active,
    ContractStatus::ExpiringSoon,
];
pub const UNPAID: &[ContractStatus] = &[
    ContractStatus::Signed,
    ContractStatus::Active,
    ContractStatus::ExpiringSoon,
];
pub const DEPRECATED: &[ContractStatus] = &[ContractStatus::ExpiringSoon];
pub const SELECTABLE: &[ContractStatus] = &[
    ContractStatus::Draft,
    ContractStatus::Generated,
    ContractStatus::Sent,
    ContractStatus::Signed,
    ContractStatus::Invoiced,
    ContractStatus::PaidActive,
    ContractStatus::Expired,
    ContractStatus::Terminated,
];

const STATUSES: &[ContractStatus] = &[
    ContractStatus::Draft,
    ContractStatus::Generated,
    ContractStatus::Sent,
    ContractStatus::Signed,
    ContractStatus::Invoiced,
    ContractStatus::PaidActive,
    ContractStatus::Active,
    ContractStatus::ExpiringSoon,
    ContractStatus::Expired,
    ContractStatus::Terminated,
];
const LABELS: [(&str, &str, &str); 10] = [
    ("Draft", "secondary", "Nháp"),
    ("Generated", "info", "Đã sinh file"),
    ("Sent", "primary", "Đã gửi"),
    ("Signed", "success", "Đã ký"),
    ("Invoiced", "info", "Đã xuất hóa đơn"),
    ("PaidActive", "success", "Đã thanh toán"),
    ("Active", "success", "Hiệu lực"),
    ("ExpiringSoon", "warning", "Sắp hết hạn"),
    ("Expired", "danger", "Đã hết hạn"),
    ("Terminated", "dark", "Thanh lý"),
];
pub fn status_label(value: &str) -> &str {
    LABELS
        .iter()
        .find(|row| row.0 == value)
        .map_or(value, |row| row.2)
}
pub fn status_color(value: &str) -> &'static str {
    LABELS
        .iter()
        .find(|row| row.0 == value)
        .map_or("secondary", |row| row.1)
}

/// Port của app/services/lifecycle.py::parse_status.
pub fn parse_status(value: &str) -> Result<ContractStatus, CoreError> {
    let status = ContractStatus::from_str(value).unwrap_or_else(|never| match never {});
    if matches!(status, ContractStatus::Unknown(_)) {
        Err(CoreError::Invalid(format!(
            "Trạng thái không hợp lệ: '{value}'"
        )))
    } else {
        Ok(status)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Evidence {
    pub has_invoice: bool,
    pub has_payment_slip: bool,
}

pub fn valid_targets(from: &ContractStatus) -> Vec<ContractStatus> {
    let names: &[&str] = match from {
        ContractStatus::Draft => &["Generated"],
        ContractStatus::Generated => &["Sent", "Signed"],
        ContractStatus::Sent => &["Signed"],
        ContractStatus::Signed => &["Invoiced", "Terminated"],
        ContractStatus::Invoiced => &["PaidActive", "Terminated"],
        ContractStatus::PaidActive => &["Expired", "Terminated"],
        ContractStatus::Active | ContractStatus::ExpiringSoon => {
            &["Expired", "Invoiced", "PaidActive", "Terminated"]
        }
        ContractStatus::Expired => &["Terminated"],
        _ => &[],
    };
    names
        .iter()
        .filter_map(|name| STATUSES.iter().find(|status| status.as_str() == *name))
        .cloned()
        .collect()
}

/// Port của app/services/lifecycle.py::validate_transition.
pub fn validate_transition(
    from: &ContractStatus,
    to: &str,
    evidence: Evidence,
) -> Result<ContractStatus, CoreError> {
    let target = parse_status(to)?;
    let allowed = valid_targets(from);
    if !allowed.contains(&target) {
        let display = if allowed.is_empty() {
            "không có".to_owned()
        } else {
            format!(
                "[{}]",
                allowed
                    .iter()
                    .map(|status| format!("'{}'", status.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        return Err(CoreError::Invalid(format!(
            "Không thể chuyển từ {} sang {}. Cho phép: {display}",
            from.as_str(),
            target.as_str()
        )));
    }
    match &target {
        ContractStatus::Invoiced if !evidence.has_invoice => {
            return Err(CoreError::Invalid(
                "Vui lòng upload file hóa đơn PDF trước khi chuyển sang 'Đã xuất hóa đơn'.".into(),
            ));
        }
        ContractStatus::PaidActive if !evidence.has_payment_slip => {
            return Err(CoreError::Invalid(
                "Vui lòng upload file ủy nhiệm chi trước khi chuyển sang 'Đã thanh toán'.".into(),
            ));
        }
        _ => {}
    }
    Ok(target)
}

pub fn days_to_expiry(end: Option<NaiveDate>, today: NaiveDate) -> Option<i64> {
    end.map(|date| (date - today).num_days())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Expired,
    Critical,
    Warning,
    Info,
}

pub fn expiry_bucket(end: Option<NaiveDate>, today: NaiveDate) -> Option<Bucket> {
    match days_to_expiry(end, today)? {
        days if days <= 0 => Some(Bucket::Expired),
        1..=7 => Some(Bucket::Critical),
        8..=14 => Some(Bucket::Warning),
        15..=30 => Some(Bucket::Info),
        _ => None,
    }
}

pub fn is_expiring_soon(end: Option<NaiveDate>, today: NaiveDate) -> bool {
    days_to_expiry(end, today).is_some_and(|days| (1..=30).contains(&days))
}
