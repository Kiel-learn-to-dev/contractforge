use std::collections::BTreeSet;

use cf_db::{paths::resolve_stored, store::Store};

use crate::{ServiceError, ctx::Ctx};

pub fn cleanup_orphan_template_files(ctx: &Ctx) -> Result<usize, ServiceError> {
    let directory = &ctx.paths().templates;
    if !directory.exists() {
        return Ok(0);
    }
    let referenced = ctx
        .store()
        .templates
        .values()
        .filter_map(|template| template.file_path.as_deref())
        .map(|path| resolve_stored(&ctx.paths().root, path))
        .collect::<BTreeSet<_>>();
    let mut removed = 0;
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && !referenced.contains(&path) {
            std::fs::remove_file(path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

pub fn cleanup_in_store(store: &Store, root: &std::path::Path) -> BTreeSet<std::path::PathBuf> {
    store
        .templates
        .values()
        .filter_map(|template| template.file_path.as_deref())
        .map(|path| resolve_stored(root, path))
        .collect()
}
