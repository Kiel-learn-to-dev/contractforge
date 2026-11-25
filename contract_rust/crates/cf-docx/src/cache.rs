use std::collections::HashMap;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use crate::{DocxError, Template};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Default)]
pub struct TemplateCache {
    entries: HashMap<Key, (Arc<Template>, u64)>,
    tick: u64,
}

impl TemplateCache {
    pub fn get_or_compile(&mut self, path: &Path) -> Result<Arc<Template>, DocxError> {
        let metadata =
            std::fs::metadata(path).map_err(|error| DocxError::Open(error.to_string()))?;
        let key = Key {
            path: path.to_path_buf(),
            len: metadata.len(),
            modified: metadata.modified().ok(),
        };
        self.tick = self.tick.wrapping_add(1);
        if let Some((template, last_used)) = self.entries.get_mut(&key) {
            *last_used = self.tick;
            return Ok(Arc::clone(template));
        }
        let bytes = std::fs::read(path).map_err(|error| DocxError::Open(error.to_string()))?;
        let template = Arc::new(Template::compile(&bytes)?);
        if self.entries.len() >= 32
            && let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone())
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(key, (Arc::clone(&template), self.tick));
        Ok(template)
    }
}
