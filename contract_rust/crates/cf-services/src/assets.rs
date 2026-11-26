//! Packaged defaults. An executable-adjacent file takes precedence over its embedded copy.

use std::fs;

const SAMPLE_CONTRACT: &[u8] =
    include_bytes!("../../../assets/default_templates/sample_service_contract.docx");
const SAMPLE_FORM_08A: &[u8] =
    include_bytes!("../../../assets/default_templates/sample_form_08a.docx");
const SAMPLE_REQUEST_LETTER: &[u8] =
    include_bytes!("../../../assets/default_templates/sample_request_letter.docx");
const SAMPLE_REQUEST_FORM: &[u8] =
    include_bytes!("../../../assets/default_templates/sample_request_form.docx");
const SAMPLE_REQUEST_DETAILED: &[u8] =
    include_bytes!("../../../assets/default_templates/sample_request_detailed.docx");
const SAMPLE_QUOTE_COMPACT: &[u8] =
    include_bytes!("../../../assets/default_templates/sample_quote_compact.docx");
const SAMPLE_QUOTE_FORMAL: &[u8] =
    include_bytes!("../../../assets/default_templates/sample_quote_formal.docx");
const SAMPLE_QUOTE_LANDSCAPE: &[u8] =
    include_bytes!("../../../assets/default_templates/sample_quote_landscape.docx");
const SAMPLE_QUOTE_FEATURES: &[u8] =
    include_bytes!("../../../assets/default_templates/sample_quote_features.docx");
const SAMPLE_QUOTE_TERMS: &[u8] =
    include_bytes!("../../../assets/default_templates/sample_quote_terms.docx");

/// Resolve a default document from `<exe>/assets/default_templates` or the embedded copy.
pub fn default_template_bytes(filename: &str) -> Option<Vec<u8>> {
    let (override_names, embedded) = match filename {
        "sample_service_contract.docx" => (&[filename][..], SAMPLE_CONTRACT),
        "sample_form_08a.docx" => (&[filename][..], SAMPLE_FORM_08A),
        "sample_request_letter.docx" => (&[filename][..], SAMPLE_REQUEST_LETTER),
        "sample_request_form.docx" => (&[filename][..], SAMPLE_REQUEST_FORM),
        "sample_request_detailed.docx" => (&[filename][..], SAMPLE_REQUEST_DETAILED),
        "sample_quote_compact.docx" => (&[filename][..], SAMPLE_QUOTE_COMPACT),
        "sample_quote_formal.docx" => (&[filename][..], SAMPLE_QUOTE_FORMAL),
        "sample_quote_landscape.docx" => (&[filename][..], SAMPLE_QUOTE_LANDSCAPE),
        "sample_quote_features.docx" => (&[filename][..], SAMPLE_QUOTE_FEATURES),
        "sample_quote_terms.docx" => (&[filename][..], SAMPLE_QUOTE_TERMS),
        "template_08A.docx" => (&[filename, "sample_form_08a.docx"][..], SAMPLE_FORM_08A),
        _ => return None,
    };

    if let Some(directory) = std::env::current_exe().ok().and_then(|path| {
        path.parent()
            .map(|parent| parent.join("assets/default_templates"))
    }) {
        for name in override_names {
            let path = directory.join(name);
            if path.is_file()
                && let Ok(bytes) = fs::read(path)
            {
                return Some(bytes);
            }
        }
    }

    Some(embedded.to_vec())
}
