use std::collections::BTreeMap;

use cf_core::render_context::{CANONICAL_MAP, ROW_PLACEHOLDERS};

/// Xây mapping mặc định cho các placeholder mà bộ phân tích phát hiện.
pub fn default_mapping(placeholders: &[String]) -> BTreeMap<String, String> {
    let known = CANONICAL_MAP
        .iter()
        .chain(ROW_PLACEHOLDERS.iter())
        .map(|(placeholder, field)| (*placeholder, *field))
        .collect::<BTreeMap<_, _>>();
    placeholders
        .iter()
        .map(|placeholder| {
            (
                placeholder.clone(),
                known
                    .get(placeholder.as_str())
                    .copied()
                    .unwrap_or("")
                    .to_owned(),
            )
        })
        .collect()
}
