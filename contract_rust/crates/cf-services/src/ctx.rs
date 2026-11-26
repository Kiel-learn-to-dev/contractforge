use cf_db::{clock::Clock, paths::DataPaths, store::Store};
use cf_docx::TemplateCache;

/// Trạng thái dùng chung của mọi service: Store, thư mục dữ liệu, đồng hồ, cache mẫu.
pub struct Ctx {
    store: Store,
    paths: DataPaths,
    clock: Box<dyn Clock>,
    templates: TemplateCache,
}

impl Ctx {
    pub fn new(store: Store, paths: DataPaths, clock: Box<dyn Clock>) -> Self {
        Self {
            store,
            paths,
            clock,
            templates: TemplateCache::default(),
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    pub fn paths(&self) -> &DataPaths {
        &self.paths
    }

    pub fn clock(&self) -> &dyn Clock {
        self.clock.as_ref()
    }

    pub fn templates_mut(&mut self) -> &mut TemplateCache {
        &mut self.templates
    }
}
