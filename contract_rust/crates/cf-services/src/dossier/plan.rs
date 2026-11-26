use std::{collections::BTreeMap, path::PathBuf};

use cf_core::{lifecycle::ACTIVE_LIFECYCLE, names::sanitize_for_filename};
use cf_db::{
    model::{Contract, Customer},
    paths::DataPaths,
    store::Store,
};

use crate::ServiceError;

pub const CONTRACT_BLOCK_START: usize = 4;
pub const SLOTS_PER_PRODUCT: usize = 2;
pub const EXTRA_START: usize = 90;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub key: String,
    pub label: String,
    pub source: String,
    pub filename: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotResult {
    pub order: usize,
    pub slot: Slot,
    pub source_path: Option<PathBuf>,
    pub dest_name: String,
    pub size: u64,
    pub note: String,
}

impl SlotResult {
    pub fn ok(&self) -> bool {
        self.source_path.is_some()
    }
    pub fn missing_required(&self) -> bool {
        self.slot.required && !self.ok()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DossierPlan {
    pub customer: Customer,
    pub contracts: Vec<Contract>,
    pub product_labels: BTreeMap<i64, (String, String)>,
    pub dest_dir: PathBuf,
    pub slots: Vec<SlotResult>,
}

impl DossierPlan {
    pub fn ready_count(&self) -> usize {
        self.slots.iter().filter(|slot| slot.ok()).count()
    }
    pub fn missing_required(&self) -> Vec<&SlotResult> {
        self.slots
            .iter()
            .filter(|slot| slot.missing_required())
            .collect()
    }
    pub fn is_complete(&self) -> bool {
        self.missing_required().is_empty()
    }
}

pub fn resolve_stored_path(paths: &DataPaths, stored: Option<&str>) -> Option<PathBuf> {
    stored.map(|path| paths.resolve_stored(path))
}

pub fn dossier_dir_for(paths: &DataPaths, customer: &Customer) -> PathBuf {
    let name = customer
        .short_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or(&customer.legal_name);
    let slug = nonempty_slug(name, 40, "KHACH_HANG");
    let code = nonempty_slug(&customer.code, 15, &format!("ID{}", customer.id));
    paths.dossier.join(format!("{code}_{slug}"))
}

fn nonempty_slug(value: &str, max_len: usize, fallback: &str) -> String {
    let slug = sanitize_for_filename(value, max_len);
    if slug.is_empty() {
        fallback.to_owned()
    } else {
        slug
    }
}

pub fn contracts_for_dossier(store: &Store, customer_id: i64) -> Vec<Contract> {
    let mut contracts = store
        .contracts
        .values()
        .filter(|contract| contract.customer_id == customer_id)
        .filter(|contract| {
            ACTIVE_LIFECYCLE.contains(&contract.status) || contract.signed_pdf_path.is_some()
        })
        .cloned()
        .collect::<Vec<_>>();
    contracts.sort_by(|a, b| {
        a.contract_number
            .cmp(&b.contract_number)
            .then(a.id.cmp(&b.id))
    });
    contracts
}

pub fn build_plan(
    store: &Store,
    paths: &DataPaths,
    customer_id: i64,
) -> Result<DossierPlan, ServiceError> {
    let customer = store.customers.get(&customer_id).cloned().ok_or_else(|| {
        ServiceError::NotFound(format!("Không tìm thấy khách hàng id={customer_id}"))
    })?;
    let contracts = contracts_for_dossier(store, customer_id);
    let mut documents = store
        .documents
        .values()
        .filter(|document| document.customer_id == customer_id)
        .collect::<Vec<_>>();
    documents.sort_by_key(|document| document.id);

    // The Python implementation uses a dict comprehension: the newest row for
    // a fixed doc_type wins, while all "other" rows retain their id order.
    let mut fixed = BTreeMap::new();
    let mut others = Vec::new();
    for document in documents {
        if document.doc_type == "other" {
            others.push(document);
        } else {
            fixed.insert(document.doc_type.as_str(), document);
        }
    }

    let mut slots = Vec::new();
    for (order, key, label, doc_type, filename) in [
        (
            1,
            "cccd_front",
            "CCCD mặt trước",
            "cccd_front",
            "CCCD_mat_truoc",
        ),
        (2, "cccd_back", "CCCD mặt sau", "cccd_back", "CCCD_mat_sau"),
        (
            3,
            "establishment",
            "Quyết định thành lập",
            "establishment",
            "Quyet_dinh_thanh_lap",
        ),
    ] {
        let slot = Slot {
            key: key.into(),
            label: label.into(),
            source: format!("customer_doc:{doc_type}"),
            filename: filename.into(),
            required: true,
        };
        let document = fixed.get(doc_type).copied();
        let (source, note) = document.map_or((None, "Chưa upload"), |document| {
            source_path(paths, Some(&document.file_path), "File không còn trên đĩa")
        });
        slots.push(fill(order, slot, source, note));
    }

    let mut product_ids = store.products.keys().copied().collect::<Vec<_>>();
    product_ids.sort_unstable();
    let positions = product_ids
        .into_iter()
        .enumerate()
        .map(|(position, id)| (id, position))
        .collect::<BTreeMap<_, _>>();
    let mut seen_per_product = BTreeMap::<Option<i64>, usize>::new();
    for contract in &contracts {
        let position = contract
            .product_id
            .and_then(|id| positions.get(&id).copied())
            .unwrap_or(positions.len());
        let base = CONTRACT_BLOCK_START + position * SLOTS_PER_PRODUCT;
        let seen = seen_per_product.entry(contract.product_id).or_default();
        let suffix = if *seen == 0 {
            String::new()
        } else {
            format!("_{}", *seen + 1)
        };
        *seen += 1;
        let product = contract.product_id.and_then(|id| store.products.get(&id));
        let product_code =
            nonempty_slug(product.map_or("KHAC", |product| &product.code), 20, "KHAC");
        let product_name =
            product.map_or("Không rõ sản phẩm", |product| product.name.as_str());

        for (offset, (field, label, stem, required)) in [
            ("signed_pdf", "Hợp đồng đã ký", "Hop_dong", true),
            ("invoice_pdf", "Hoá đơn", "Hoa_don", false),
        ]
        .into_iter()
        .enumerate()
        {
            let slot = Slot {
                key: format!("contract_{}_{}", contract.id, field),
                label: format!("{label} — {product_name} ({})", contract.contract_number),
                source: format!("contract:{field}"),
                filename: format!("{stem}_{product_code}{suffix}"),
                required,
            };
            let stored = match field {
                "signed_pdf" => contract.signed_pdf_path.as_deref(),
                _ => contract.invoice_pdf_path.as_deref(),
            };
            let (source, note) = source_path(paths, stored, "Hợp đồng chưa có file này");
            slots.push(fill(base + offset, slot, source, note));
        }
    }

    for (index, document) in others.into_iter().enumerate() {
        let label = document.label.as_deref().unwrap_or("Giấy tờ khác").trim();
        let slot = Slot {
            key: format!("other_{}", document.id),
            label: label.to_owned(),
            source: format!("customer_doc_id:{}", document.id),
            filename: nonempty_slug(label, 40, "Giay_to_khac"),
            required: false,
        };
        let (source, note) =
            source_path(paths, Some(&document.file_path), "File không còn trên đĩa");
        slots.push(fill(EXTRA_START + index, slot, source, note));
    }
    slots.sort_by(|a, b| {
        a.order
            .cmp(&b.order)
            .then(a.slot.filename.cmp(&b.slot.filename))
    });

    let product_labels = store
        .products
        .iter()
        .map(|(id, product)| (*id, (product.code.clone(), product.name.clone())))
        .collect();
    Ok(DossierPlan {
        dest_dir: dossier_dir_for(paths, &customer),
        customer,
        contracts,
        product_labels,
        slots,
    })
}

fn source_path(
    paths: &DataPaths,
    stored: Option<&str>,
    missing_note: &'static str,
) -> (Option<PathBuf>, &'static str) {
    let Some(path) = resolve_stored_path(paths, stored) else {
        return (None, missing_note);
    };
    if path.is_file() {
        (Some(path), "")
    } else {
        (None, "File không còn trên đĩa")
    }
}

fn fill(order: usize, slot: Slot, source_path: Option<PathBuf>, note: &'static str) -> SlotResult {
    let (dest_name, size, note) = match source_path
        .as_deref()
        .and_then(|path| path.metadata().ok().map(|metadata| (path, metadata.len())))
    {
        Some((path, size)) => {
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_lowercase();
            (
                format!(
                    "{order:02}_{}{}",
                    slot.filename,
                    if extension.is_empty() {
                        String::new()
                    } else {
                        format!(".{extension}")
                    }
                ),
                size,
                String::new(),
            )
        }
        None => (
            String::new(),
            0,
            if source_path.is_some() {
                "File không còn trên đĩa"
            } else {
                note
            }
            .into(),
        ),
    };
    let source_path = if dest_name.is_empty() {
        None
    } else {
        source_path
    };
    SlotResult {
        order,
        slot,
        source_path,
        dest_name,
        size,
        note,
    }
}
