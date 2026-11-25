use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPaths {
    pub root: PathBuf,
    pub db: PathBuf,
    pub templates: PathBuf,
    pub signed_scans: PathBuf,
    pub customer_docs: PathBuf,
    pub invoice_docs: PathBuf,
    pub payment_slips: PathBuf,
    pub output_contracts: PathBuf,
    pub output_bundles: PathBuf,
    pub output_batch: PathBuf,
    pub dossier: PathBuf,
    pub backups: PathBuf,
    pub uploads: PathBuf,
    pub logs: PathBuf,
}

/// Port của app/paths.py::_resolve_data_root.
pub fn resolve_data_root(
    env: Option<&str>,
    localappdata: Option<&Path>,
    exe_dir: Option<&Path>,
) -> PathBuf {
    if let Some(path) = env.filter(|path| !path.trim().is_empty()) {
        return PathBuf::from(path);
    }
    let local_root = localappdata.map(|path| path.join("ContractForge"));
    if let Some(path) = local_root
        .as_ref()
        .filter(|path| path.join("contract_manager.db").is_file())
    {
        return path.clone();
    }
    let legacy_root = exe_dir.map(|path| path.join("data"));
    if let Some(path) = legacy_root
        .as_ref()
        .filter(|path| path.join("contract_manager.db").is_file())
    {
        return path.clone();
    }
    local_root.unwrap_or_else(|| PathBuf::from("ContractForge"))
}

/// Port của app/paths.py::resolve_stored.
pub fn resolve_stored(root: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

impl DataPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            db: root.join("contract_manager.db"),
            templates: root.join("uploads/templates"),
            signed_scans: root.join("uploads/signed_scans"),
            customer_docs: root.join("uploads/customer_docs"),
            invoice_docs: root.join("uploads/invoice_docs"),
            payment_slips: root.join("uploads/payment_slips"),
            output_contracts: root.join("outputs/contracts"),
            output_bundles: root.join("outputs/bundles"),
            output_batch: root.join("outputs/batch"),
            dossier: root.join("dau-noi"),
            backups: root.join("backups"),
            uploads: root.join("uploads"),
            logs: root.join("logs"),
            root,
        }
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        for path in [
            &self.templates,
            &self.signed_scans,
            &self.customer_docs,
            &self.invoice_docs,
            &self.payment_slips,
            &self.output_contracts,
            &self.output_bundles,
            &self.output_batch,
            &self.dossier,
            &self.backups,
            &self.logs,
        ] {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }

    /// Port của app/paths.py::resolve_stored.
    pub fn resolve_stored(&self, raw: &str) -> PathBuf {
        resolve_stored(&self.root, raw)
    }
}
