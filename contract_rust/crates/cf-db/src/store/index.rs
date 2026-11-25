use std::collections::{BTreeMap, HashMap};

use cf_core::text::search_key;

use crate::{
    model::{
        Contract, ContractEvent, ContractItem, Customer, CustomerDocument, CustomerUnit, Product,
    },
    store::Store,
};

#[derive(Debug, Default)]
pub struct Indexes {
    pub contract_by_number: HashMap<String, i64>,
    pub contracts_by_customer: BTreeMap<i64, Vec<i64>>,
    pub children_by_parent: BTreeMap<i64, Vec<i64>>,
    pub renewals_by_old: BTreeMap<i64, Vec<i64>>,
    pub items_by_contract: BTreeMap<i64, Vec<i64>>,
    pub units_by_customer: BTreeMap<i64, Vec<i64>>,
    pub docs_by_customer: BTreeMap<i64, Vec<i64>>,
    pub events_by_contract: BTreeMap<i64, Vec<i64>>,
    pub customer_by_code: HashMap<String, i64>,
    pub product_by_code: HashMap<String, i64>,
    pub customer_search_key: HashMap<i64, String>,
    pub contract_search_key: HashMap<i64, String>,
}

impl Indexes {
    pub fn build(store: &Store) -> Self {
        let mut indexes = Self::default();
        for customer in store.customers.values() {
            indexes
                .customer_by_code
                .insert(customer.code.clone(), customer.id);
            indexes.customer_search_key.insert(
                customer.id,
                search_key(&format!(
                    "{} {} {}",
                    customer.code,
                    customer.legal_name,
                    customer.short_name.as_deref().unwrap_or_default()
                )),
            );
        }
        for product in store.products.values() {
            indexes
                .product_by_code
                .insert(product.code.clone(), product.id);
        }
        for contract in store.contracts.values() {
            indexes
                .contract_by_number
                .insert(contract.contract_number.clone(), contract.id);
            indexes
                .contracts_by_customer
                .entry(contract.customer_id)
                .or_default()
                .push(contract.id);
            if let Some(parent) = contract.parent_contract_id {
                indexes
                    .children_by_parent
                    .entry(parent)
                    .or_default()
                    .push(contract.id);
            }
            if let Some(old) = contract.renewed_from_id {
                indexes
                    .renewals_by_old
                    .entry(old)
                    .or_default()
                    .push(contract.id);
            }
            let customer_name = store
                .customers
                .get(&contract.customer_id)
                .map_or("", |customer| customer.legal_name.as_str());
            indexes.contract_search_key.insert(
                contract.id,
                search_key(&format!(
                    "{} {} {}",
                    contract.contract_number,
                    contract.notes.as_deref().unwrap_or_default(),
                    customer_name
                )),
            );
        }
        for item in store.items.values() {
            indexes
                .items_by_contract
                .entry(item.contract_id)
                .or_default()
                .push(item.id);
        }
        for unit in store.units.values() {
            indexes
                .units_by_customer
                .entry(unit.customer_id)
                .or_default()
                .push(unit.id);
        }
        for doc in store.documents.values() {
            indexes
                .docs_by_customer
                .entry(doc.customer_id)
                .or_default()
                .push(doc.id);
        }
        for event in store.events.values() {
            indexes
                .events_by_contract
                .entry(event.contract_id)
                .or_default()
                .push(event.id);
        }
        for ids in indexes.items_by_contract.values_mut() {
            ids.sort_by_key(|id| store.items.get(id).map_or(i64::MAX, |item| item.sort_order));
        }
        for ids in indexes.units_by_customer.values_mut() {
            ids.sort_by_key(|id| store.units.get(id).map(|unit| (unit.sort_order, unit.id)));
        }
        for ids in indexes.docs_by_customer.values_mut() {
            ids.sort_by_key(|id| {
                store
                    .documents
                    .get(id)
                    .map(|doc| (doc.doc_type.clone(), doc.id))
            });
        }
        for ids in indexes.events_by_contract.values_mut() {
            ids.sort_by_key(|id| store.events.get(id).map(|event| event.occurred_at));
        }
        indexes
    }

    pub fn contract_by_number<'a>(&self, store: &'a Store, number: &str) -> Option<&'a Contract> {
        self.contract_by_number
            .get(number)
            .and_then(|id| store.contracts.get(id))
    }

    pub fn contracts_of_customer<'a>(
        &self,
        store: &'a Store,
        customer_id: i64,
    ) -> Vec<&'a Contract> {
        self.contracts_by_customer
            .get(&customer_id)
            .into_iter()
            .flatten()
            .filter_map(|id| store.contracts.get(id))
            .collect()
    }

    pub fn children_of<'a>(&self, store: &'a Store, contract_id: i64) -> Vec<&'a Contract> {
        self.children_by_parent
            .get(&contract_id)
            .into_iter()
            .flatten()
            .filter_map(|id| store.contracts.get(id))
            .collect()
    }

    pub fn renewals_of<'a>(&self, store: &'a Store, contract_id: i64) -> Vec<&'a Contract> {
        self.renewals_by_old
            .get(&contract_id)
            .into_iter()
            .flatten()
            .filter_map(|id| store.contracts.get(id))
            .collect()
    }

    pub fn items_of<'a>(&self, store: &'a Store, contract_id: i64) -> Vec<&'a ContractItem> {
        self.items_by_contract
            .get(&contract_id)
            .into_iter()
            .flatten()
            .filter_map(|id| store.items.get(id))
            .collect()
    }

    pub fn units_of<'a>(&self, store: &'a Store, customer_id: i64) -> Vec<&'a CustomerUnit> {
        self.units_by_customer
            .get(&customer_id)
            .into_iter()
            .flatten()
            .filter_map(|id| store.units.get(id))
            .collect()
    }

    pub fn docs_of<'a>(&self, store: &'a Store, customer_id: i64) -> Vec<&'a CustomerDocument> {
        self.docs_by_customer
            .get(&customer_id)
            .into_iter()
            .flatten()
            .filter_map(|id| store.documents.get(id))
            .collect()
    }

    pub fn events_of<'a>(&self, store: &'a Store, contract_id: i64) -> Vec<&'a ContractEvent> {
        self.events_by_contract
            .get(&contract_id)
            .into_iter()
            .flatten()
            .filter_map(|id| store.events.get(id))
            .collect()
    }

    pub fn customer_by_code<'a>(&self, store: &'a Store, code: &str) -> Option<&'a Customer> {
        self.customer_by_code
            .get(code)
            .and_then(|id| store.customers.get(id))
    }

    pub fn product_by_code<'a>(&self, store: &'a Store, code: &str) -> Option<&'a Product> {
        self.product_by_code
            .get(code)
            .and_then(|id| store.products.get(id))
    }
}
