//! Chuỗi và nhãn đã định dạng để đưa lên màn hình (05_GIAO_DIEN §1: tiền bằng `fmt_currency`,
//! ngày `dd/mm/yyyy`, nhãn và màu trạng thái từ cf-core). Không có nghiệp vụ ở đây.

use cf_core::{
    dates::format_date_vn,
    fmt::fmt_currency,
    lifecycle::{ContractStatus, status_color},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;

/// Bảy biến thể màu của nhãn; tên trùng với tên màu trong `cf_core::lifecycle`
/// và với các nhánh `variant == ...` trong `ui/components/common.slint`.
pub const VARIANTS: [&str; 7] = [
    "secondary",
    "info",
    "primary",
    "success",
    "warning",
    "danger",
    "dark",
];

/// Nhãn trạng thái đã sẵn sàng hiển thị.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badge {
    pub label: String,
    pub variant: &'static str,
}

/// Đưa tên màu của cf-core về một trong bảy biến thể. Tên lạ thì dùng `secondary`
/// (cùng quy ước với `status_color`).
pub fn variant_of(color: &str) -> &'static str {
    VARIANTS
        .iter()
        .copied()
        .find(|variant| *variant == color)
        .unwrap_or("secondary")
}

/// Nhãn và màu của một trạng thái hợp đồng.
pub fn status_badge(status: &ContractStatus) -> Badge {
    Badge {
        label: status.label().to_owned(),
        variant: variant_of(status_color(status.as_str())),
    }
}

#[allow(dead_code)] // dùng từ màn hình danh sách (T4.B06)
/// Tiền hiển thị theo `fmt_currency`, ví dụ `1.250.000`. Không có giá trị thì chuỗi rỗng.
pub fn money_text(value: Option<Decimal>) -> String {
    fmt_currency(value)
}

#[allow(dead_code)] // dùng từ màn hình danh sách (T4.B06)
/// Ngày hiển thị theo `dd/mm/yyyy`. Không có ngày thì chuỗi rỗng.
pub fn date_text(date: Option<NaiveDate>) -> String {
    format_date_vn(date)
}

#[allow(dead_code)]
/// Mười trạng thái theo thứ tự của cf-core, dùng cho màn hình thử nhãn.
pub fn all_statuses() -> Vec<ContractStatus> {
    vec![
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
    ]
}

#[cfg(test)]
mod tests {
    use super::{VARIANTS, all_statuses, date_text, money_text, status_badge, variant_of};
    use cf_core::lifecycle::ContractStatus;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

    #[test]
    fn every_status_gets_a_known_variant_and_label() {
        let statuses = all_statuses();
        assert_eq!(statuses.len(), 10);
        for status in &statuses {
            let badge = status_badge(status);
            assert!(VARIANTS.contains(&badge.variant), "{badge:?}");
            assert!(!badge.label.is_empty());
        }
    }

    #[test]
    fn known_statuses_match_core_labels_and_colors() {
        let draft = status_badge(&ContractStatus::Draft);
        assert_eq!(draft.label, "Nháp");
        assert_eq!(draft.variant, "secondary");

        let expired = status_badge(&ContractStatus::Expired);
        assert_eq!(expired.label, "Đã hết hạn");
        assert_eq!(expired.variant, "danger");

        let terminated = status_badge(&ContractStatus::Terminated);
        assert_eq!(terminated.label, "Thanh lý");
        assert_eq!(terminated.variant, "dark");
    }

    #[test]
    fn unknown_color_falls_back_to_secondary() {
        assert_eq!(variant_of("lạ"), "secondary");
        assert_eq!(variant_of("success"), "success");
    }

    #[test]
    fn money_and_date_text_follow_core_formats() {
        assert_eq!(money_text(Some(Decimal::new(1_250_000, 0))), "1.250.000");
        assert_eq!(money_text(None), "");
        let date = NaiveDate::from_ymd_opt(2026, 3, 9);
        assert_eq!(date_text(date), "09/03/2026");
        assert_eq!(date_text(None), "");
    }
}
