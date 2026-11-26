//! Cài đặt ứng dụng và hồ sơ Bên B.

use cf_core::{numbering, render_context::PartyB};
use cf_db::{change::Change, store::Store};

pub const PARTY_B_KEYS: &[&str] = &[
    "party_b_representative",
    "party_b_authorization_number",
    "party_b_authorization_date",
    "party_b_name",
    "party_b_address",
    "party_b_bank_account",
    "party_b_bank_name",
    "party_b_beneficiary",
    "party_b_tax_code",
    "party_b_title",
    "party_b_phone",
    "party_b_contact_name",
    "party_b_contact_email",
];

/// Đọc cài đặt hiện có; giá trị NULL/rỗng dùng mặc định.
pub fn get_setting(store: &Store, key: &str, default: &str) -> String {
    store
        .settings
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_owned()
}

/// Cập nhật một cài đặt trong SQLite và làm mới Store.
pub fn set_setting(store: &mut Store, key: &str, value: &str) -> Result<(), crate::ServiceError> {
    store.apply(vec![Change::SetSetting {
        key: key.to_owned(),
        value: value.to_owned(),
    }])?;
    Ok(())
}

/// Kiểm tra hai trường định danh tối thiểu trước khi tạo hợp đồng.
pub fn is_organization_profile_complete(store: &Store) -> bool {
    !get_setting(store, "party_b_name", "").trim().is_empty()
        && !get_setting(store, "party_b_representative", "")
            .trim()
            .is_empty()
}

/// Trả về toàn bộ thông tin Bên B để dựng render context.
pub fn get_party_b_settings(store: &Store) -> PartyB {
    let get = |suffix: &str| get_setting(store, &format!("party_b_{suffix}"), "");
    PartyB {
        name: get("name"),
        address: get("address"),
        bank_account: get("bank_account"),
        bank_name: get("bank_name"),
        beneficiary: get("beneficiary"),
        tax_code: get("tax_code"),
        title: get("title"),
        representative: get("representative"),
        authorization_number: get("authorization_number"),
        authorization_date: get("authorization_date"),
        phone: get("phone"),
        contact_name: get("contact_name"),
        contact_email: get("contact_email"),
    }
}

/// Lưu thông tin Bên B, trim các giá trị và ghi trong một transaction.
pub fn save_party_b(
    store: &mut Store,
    values: impl IntoIterator<Item = (String, String)>,
) -> Result<(), crate::ServiceError> {
    let changes = values
        .into_iter()
        .filter(|(key, _)| PARTY_B_KEYS.contains(&key.as_str()))
        .map(|(key, value)| Change::SetSetting {
            key,
            value: value.trim().to_owned(),
        })
        .collect();
    store.apply(changes)?;
    Ok(())
}

/// Kiểm tra khuôn phụ lục trước khi lưu để tránh cập nhật cấu hình không dùng được.
pub fn save_addendum_format(store: &mut Store, format: &str) -> Result<(), crate::ServiceError> {
    let format = format.trim();
    numbering::validate_addendum_format(format)?;
    set_setting(store, "addendum_number_format", format)
}

pub fn addendum_format(store: &Store) -> String {
    get_setting(
        store,
        "addendum_number_format",
        numbering::DEFAULT_ADDENDUM_FORMAT,
    )
}
