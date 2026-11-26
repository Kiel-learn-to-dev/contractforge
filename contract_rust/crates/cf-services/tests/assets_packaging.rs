use std::{collections::BTreeMap, fs, path::PathBuf};

use cf_db::store::Store;
use cf_services::{
    assets::default_template_bytes,
    contracts::form08a::render_form_08a,
    quotations::manual::{build_document, render_document},
};
use serde_json::json;

struct ExecutableAssetOverride {
    path: PathBuf,
    directory: PathBuf,
}

impl ExecutableAssetOverride {
    fn write(filename: &str, bytes: &[u8]) -> Self {
        let directory = std::env::current_exe()
            .expect("test executable path")
            .parent()
            .expect("test executable directory")
            .join("assets")
            .join("default_templates");
        let path = directory.join(filename);
        assert!(
            !path.exists(),
            "unexpected preexisting test override {}",
            path.display()
        );
        fs::create_dir_all(&directory).expect("create executable-adjacent test assets");
        fs::write(&path, bytes).expect("write executable-adjacent test override");
        Self { path, directory }
    }
}

impl Drop for ExecutableAssetOverride {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
        if let Some(assets) = self.directory.parent() {
            let _ = fs::remove_dir(assets);
        }
    }
}

#[test]
fn embedded_quote_and_08a_render_and_executable_overrides_take_precedence() {
    let quote = default_template_bytes("sample_quote_compact.docx").expect("embedded quote");
    let document = build_document(
        &BTreeMap::from([
            ("layout".into(), "compact".into()),
            ("issuer_name".into(), "ContractForge".into()),
            ("recipient_name".into(), "Bệnh viện thử nghiệm".into()),
            ("quote_date".into(), "26/11/2026".into()),
            (
                "items_json".into(),
                json!([{"name": "Dịch vụ thử nghiệm", "unit": "Tháng", "quantity": "1", "periods": "12", "price": "500000", "vat": "8"}]).to_string(),
            ),
        ]),
        &[],
    )
    .expect("manual quotation data");
    let rendered_quote = render_document(&quote, &document).expect("render embedded manual quote");
    assert!(
        !cf_docx::inspect::docx_texts(&rendered_quote)
            .expect("inspect quote")
            .is_empty()
    );

    let database_dir = tempfile::tempdir().expect("database directory");
    let database = cf_testkit::seeded_db(database_dir.path()).expect("seed database");
    let store = Store::load(&database).expect("load seeded database");
    let (form_08a, filename) = render_form_08a(&store, 1).expect("render embedded 08A");
    assert!(filename.ends_with(".docx"));
    assert!(!form_08a.is_empty());

    let marker = b"executable-adjacent asset wins";
    let _quote_override = ExecutableAssetOverride::write("sample_quote_compact.docx", marker);
    assert_eq!(
        default_template_bytes("sample_quote_compact.docx").as_deref(),
        Some(marker.as_slice())
    );

    let _form_override = ExecutableAssetOverride::write("template_08A.docx", marker);
    assert_eq!(
        default_template_bytes("template_08A.docx").as_deref(),
        Some(marker.as_slice())
    );

    let _canonical_form_override = ExecutableAssetOverride::write("sample_form_08a.docx", marker);
    assert_eq!(
        default_template_bytes("sample_form_08a.docx").as_deref(),
        Some(marker.as_slice())
    );
    assert!(default_template_bytes("../Cargo.toml").is_none());
}
