mod apply;
mod index;
pub use apply::ApplyResult;
pub use index::Indexes;

use std::{collections::BTreeMap, path::Path};

use rusqlite::{Connection, Row};

use crate::{
    DbError,
    model::{
        AppSetting, Contract, ContractEvent, ContractItem, ContractTemplate, Customer,
        CustomerDocument, CustomerUnit, Product, QuotationIssuer,
    },
    open,
};

#[derive(Debug)]
pub struct Store {
    pub conn: Connection,
    pub customers: BTreeMap<i64, Customer>,
    pub units: BTreeMap<i64, CustomerUnit>,
    pub documents: BTreeMap<i64, CustomerDocument>,
    pub products: BTreeMap<i64, Product>,
    pub templates: BTreeMap<i64, ContractTemplate>,
    pub contracts: BTreeMap<i64, Contract>,
    pub items: BTreeMap<i64, ContractItem>,
    pub events: BTreeMap<i64, ContractEvent>,
    pub settings: BTreeMap<String, String>,
    pub issuers: BTreeMap<i64, QuotationIssuer>,
    pub indexes: Indexes,
    pub data_version: i64,
}

fn load_map<T>(
    conn: &Connection,
    sql: &str,
    parse: impl Fn(&Row<'_>) -> Result<T, DbError>,
    id: impl Fn(&T) -> i64,
) -> Result<BTreeMap<i64, T>, DbError> {
    let mut statement = conn.prepare(sql)?;
    let mut rows = statement.query([])?;
    let mut values = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let value = parse(row)?;
        values.insert(id(&value), value);
    }
    Ok(values)
}

impl Store {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let conn = open::open(path)?;
        Self::from_connection(conn)
    }

    pub fn from_connection(conn: Connection) -> Result<Self, DbError> {
        let customers = load_map(
            &conn,
            "SELECT * FROM customers",
            Customer::from_row,
            |value| value.id,
        )?;
        let units = load_map(
            &conn,
            "SELECT * FROM customer_units",
            CustomerUnit::from_row,
            |value| value.id,
        )?;
        let documents = load_map(
            &conn,
            "SELECT * FROM customer_documents",
            CustomerDocument::from_row,
            |value| value.id,
        )?;
        let products = load_map(
            &conn,
            "SELECT * FROM products",
            Product::from_row,
            |value| value.id,
        )?;
        let templates = load_map(
            &conn,
            "SELECT * FROM contract_templates",
            ContractTemplate::from_row,
            |value| value.id,
        )?;
        let contracts = load_map(
            &conn,
            "SELECT * FROM contracts",
            Contract::from_row,
            |value| value.id,
        )?;
        let items = load_map(
            &conn,
            "SELECT * FROM contract_items",
            ContractItem::from_row,
            |value| value.id,
        )?;
        let events = load_map(
            &conn,
            "SELECT * FROM contract_events",
            ContractEvent::from_row,
            |value| value.id,
        )?;
        let issuers = load_map(
            &conn,
            "SELECT * FROM quotation_issuers",
            QuotationIssuer::from_row,
            |value| value.id,
        )?;
        let mut settings_map = BTreeMap::new();
        let mut statement = conn.prepare("SELECT * FROM app_settings")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let setting = AppSetting::from_row(row)?;
            settings_map.insert(setting.key, setting.value.unwrap_or_default());
        }
        drop(rows);
        drop(statement);
        let data_version = conn.pragma_query_value(None, "data_version", |row| row.get(0))?;
        let mut store = Self {
            conn,
            customers,
            units,
            documents,
            products,
            templates,
            contracts,
            items,
            events,
            settings: settings_map,
            issuers,
            indexes: Indexes::default(),
            data_version,
        };
        let indexes = Indexes::build(&store);
        store.indexes = indexes;
        Ok(store)
    }

    pub fn reload(&mut self) -> Result<(), DbError> {
        let customers = load_map(
            &self.conn,
            "SELECT * FROM customers",
            Customer::from_row,
            |value| value.id,
        )?;
        let units = load_map(
            &self.conn,
            "SELECT * FROM customer_units",
            CustomerUnit::from_row,
            |value| value.id,
        )?;
        let documents = load_map(
            &self.conn,
            "SELECT * FROM customer_documents",
            CustomerDocument::from_row,
            |value| value.id,
        )?;
        let products = load_map(
            &self.conn,
            "SELECT * FROM products",
            Product::from_row,
            |value| value.id,
        )?;
        let templates = load_map(
            &self.conn,
            "SELECT * FROM contract_templates",
            ContractTemplate::from_row,
            |value| value.id,
        )?;
        let contracts = load_map(
            &self.conn,
            "SELECT * FROM contracts",
            Contract::from_row,
            |value| value.id,
        )?;
        let items = load_map(
            &self.conn,
            "SELECT * FROM contract_items",
            ContractItem::from_row,
            |value| value.id,
        )?;
        let events = load_map(
            &self.conn,
            "SELECT * FROM contract_events",
            ContractEvent::from_row,
            |value| value.id,
        )?;
        let issuers = load_map(
            &self.conn,
            "SELECT * FROM quotation_issuers",
            QuotationIssuer::from_row,
            |value| value.id,
        )?;
        let mut settings = BTreeMap::new();
        let mut statement = self.conn.prepare("SELECT * FROM app_settings")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let setting = AppSetting::from_row(row)?;
            settings.insert(setting.key, setting.value.unwrap_or_default());
        }
        self.customers = customers;
        self.units = units;
        self.documents = documents;
        self.products = products;
        self.templates = templates;
        self.contracts = contracts;
        self.items = items;
        self.events = events;
        self.settings = settings;
        self.issuers = issuers;
        self.data_version = self
            .conn
            .pragma_query_value(None, "data_version", |row| row.get(0))?;
        let indexes = Indexes::build(self);
        self.indexes = indexes;
        Ok(())
    }
}
