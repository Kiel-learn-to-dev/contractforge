use std::{collections::BTreeSet, fs};

use cf_db::{change::Change, model::Contract};

use crate::{Ctx, ServiceError};

/// Port của app/services/contract_service.py::_refuse_if_has_addenda.
fn ensure_no_addenda(store: &cf_db::store::Store, contract: &Contract) -> Result<(), ServiceError> {
    let count = store
        .contracts
        .values()
        .filter(|child| child.parent_contract_id == Some(contract.id))
        .count();
    if count > 0 {
        return Err(ServiceError::Invalid(format!(
            "Không xóa được hợp đồng {}: còn {count} phụ lục. Hãy xóa các phụ lục trước.",
            contract.contract_number
        )));
    }
    Ok(())
}

/// Port của app/services/contract_service.py::delete_contract.
pub fn delete_contract(ctx: &mut Ctx, contract_id: i64) -> Result<bool, ServiceError> {
    bulk_delete(ctx, &[contract_id]).map(|count| count > 0)
}

/// Port của app/services/contract_service.py::bulk_delete.
pub fn bulk_delete(ctx: &mut Ctx, ids: &[i64]) -> Result<usize, ServiceError> {
    let (contracts, changes) = {
        let store = ctx.store();
        let requested = ids.iter().copied().collect::<BTreeSet<_>>();
        let contracts = requested
            .iter()
            .filter_map(|id| store.contracts.get(id).cloned())
            .collect::<Vec<_>>();
        for contract in &contracts {
            ensure_no_addenda(store, contract)?;
        }
        let mut changes = Vec::new();
        for contract in &contracts {
            for event in store
                .events
                .values()
                .filter(|event| event.contract_id == contract.id)
            {
                changes.push(Change::delete("contract_events", event.id));
            }
            changes.push(Change::delete("contracts", contract.id));
        }
        (contracts, changes)
    };
    if contracts.is_empty() {
        return Ok(0);
    }
    ctx.store_mut().apply(changes)?;
    for contract in &contracts {
        delete_owned_files(ctx, contract);
    }
    Ok(contracts.len())
}

fn delete_owned_files(ctx: &Ctx, contract: &Contract) {
    for path in [
        contract.output_file_path.as_deref(),
        contract.signed_pdf_path.as_deref(),
        contract.invoice_pdf_path.as_deref(),
        contract.payment_slip_path.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let _ = fs::remove_file(ctx.paths().resolve_stored(path));
    }
    let _ = fs::remove_dir_all(ctx.paths().output_bundles.join(contract.id.to_string()));
}
