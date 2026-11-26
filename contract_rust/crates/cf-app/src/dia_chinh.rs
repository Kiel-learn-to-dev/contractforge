//! Dữ liệu địa chính 2 cấp (Tỉnh/TP → Xã/Phường/Đặc khu) nhúng từ `assets/dia_chinh.json`.

use std::{collections::BTreeMap, sync::OnceLock};

use serde_json::Value;

const RAW_JSON: &str = include_str!("../../../assets/dia_chinh.json");

struct DiaChinh {
    provinces: Vec<String>,
    wards_by_province: BTreeMap<String, Vec<String>>,
}

fn data() -> &'static DiaChinh {
    static DATA: OnceLock<DiaChinh> = OnceLock::new();
    DATA.get_or_init(|| {
        let parsed: Value = serde_json::from_str(RAW_JSON).unwrap_or(Value::Null);
        let mut provinces = Vec::new();
        let mut wards_by_province = BTreeMap::new();
        if let Some(map) = parsed.as_object() {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by_key(|(code, _)| *code);
            for (_code, province) in entries {
                let Some(name) = province.get("ten").and_then(Value::as_str) else {
                    continue;
                };
                let wards = province
                    .get("xa")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|w| w.get("ten").and_then(Value::as_str).map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                provinces.push(name.to_owned());
                wards_by_province.insert(name.to_owned(), wards);
            }
        }
        DiaChinh {
            provinces,
            wards_by_province,
        }
    })
}

pub fn provinces() -> &'static [String] {
    &data().provinces
}

pub fn wards_of(province: &str) -> &'static [String] {
    data()
        .wards_by_province
        .get(province)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::{provinces, wards_of};

    #[test]
    fn loads_provinces_and_wards_from_embedded_json() {
        let provs = provinces();
        assert!(!provs.is_empty());
        assert!(provs.iter().any(|p| p == "Thành phố Hà Nội"));
        let wards = wards_of("Thành phố Hà Nội");
        assert!(wards.iter().any(|w| w == "Phường Ba Đình"));
        assert!(wards_of("Tỉnh Không Tồn Tại").is_empty());
    }
}
