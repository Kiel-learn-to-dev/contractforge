//! Engine đọc, phân tích và điền mẫu DOCX.

pub mod cache;
pub mod error;
pub mod inspect;
pub mod legacy_party_a;
pub mod package;
pub mod para;
pub mod replace;
pub mod slot;
pub mod table;
pub mod template;
pub mod write;
pub mod xml;

pub use cache::TemplateCache;
pub use error::DocxError;
pub use package::Package;
pub use para::{NodePath, joined_text, text_nodes};
pub use replace::{replace_slot, replace_values};
pub use slot::{Slot, scan};
pub use template::{Analysis, RenderData, Template};
pub use write::write_new_file;
