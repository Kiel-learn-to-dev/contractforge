-- DDL của DB cài mới do bản Python tạo (init_db + run_migrations). Sinh bởi tools/gen_fixtures.py.
-- Bản Rust chạy nguyên văn file này khi tạo DB mới.

CREATE TABLE app_settings (
	"key" VARCHAR(100) NOT NULL, 
	value TEXT, 
	PRIMARY KEY ("key")
);

CREATE TABLE contract_events (
	id INTEGER NOT NULL, 
	contract_id INTEGER NOT NULL, 
	event_type VARCHAR(50) NOT NULL, 
	description TEXT, 
	actor VARCHAR(100), 
	meta_json TEXT, 
	occurred_at DATETIME NOT NULL, 
	PRIMARY KEY (id), 
	FOREIGN KEY(contract_id) REFERENCES contracts (id)
);

CREATE TABLE contract_items (
	id INTEGER NOT NULL, 
	contract_id INTEGER NOT NULL, 
	product_id INTEGER, 
	sort_order INTEGER NOT NULL, 
	description VARCHAR(300) NOT NULL, 
	unit_label VARCHAR(50), 
	quantity NUMERIC(18, 2) NOT NULL, 
	period_count NUMERIC(10, 4) NOT NULL, 
	unit_price_pre_vat NUMERIC(18, 2), 
	vat_rate NUMERIC(5, 2) NOT NULL, 
	amount_pre_vat NUMERIC(18, 0) NOT NULL, 
	vat_amount NUMERIC(18, 0) NOT NULL, 
	amount_total NUMERIC(18, 0) NOT NULL, 
	note TEXT, 
	PRIMARY KEY (id), 
	FOREIGN KEY(contract_id) REFERENCES contracts (id) ON DELETE CASCADE, 
	FOREIGN KEY(product_id) REFERENCES products (id)
);

CREATE TABLE contract_templates (
	id INTEGER NOT NULL, 
	code VARCHAR(50) NOT NULL, 
	name VARCHAR(200) NOT NULL, 
	contract_type VARCHAR(80) NOT NULL, 
	product_group VARCHAR(50), 
	file_path VARCHAR(500), 
	file_name VARCHAR(200), 
	mode VARCHAR(20) NOT NULL, 
	detected_placeholders TEXT, 
	field_mapping TEXT, 
	description TEXT, 
	is_active BOOLEAN NOT NULL, 
	created_at DATETIME NOT NULL, 
	updated_at DATETIME NOT NULL, 
	PRIMARY KEY (id)
);

CREATE TABLE contracts (
	id INTEGER NOT NULL, 
	contract_number VARCHAR(100) NOT NULL, 
	status VARCHAR(12) NOT NULL, 
	customer_id INTEGER NOT NULL, 
	product_id INTEGER, 
	template_id INTEGER, 
	sign_date DATE, 
	start_date DATE, 
	end_date DATE, 
	month_count INTEGER, 
	sign_location VARCHAR(300), 
	unit_count INTEGER, 
	quantity NUMERIC(18, 2), 
	unit_price_pre_vat NUMERIC(18, 0), 
	vat_rate NUMERIC(5, 2), 
	unit_price_vat NUMERIC(18, 0), 
	total_amount NUMERIC(18, 0), 
	amount_text VARCHAR(500), 
	pricing_description TEXT, 
	party_b_representative VARCHAR(150), 
	party_b_authorization_number VARCHAR(100), 
	party_b_authorization_date VARCHAR(50), 
	acceptance_date DATE, 
	liquidation_date DATE, 
	output_file_path VARCHAR(500), 
	output_file_name VARCHAR(200), 
	signed_pdf_path VARCHAR(500), 
	signed_pdf_name VARCHAR(200), 
	signed_pdf_uploaded_at DATETIME, 
	invoice_date DATE, 
	invoice_pdf_path VARCHAR(500), 
	invoice_pdf_name VARCHAR(200), 
	invoice_pdf_uploaded_at DATETIME, 
	payment_date DATE, 
	payment_slip_path VARCHAR(500), 
	payment_slip_name VARCHAR(200), 
	payment_slip_uploaded_at DATETIME, 
	selected_unit_names TEXT, 
	render_data TEXT, 
	notes TEXT, 
	is_batch BOOLEAN, 
	batch_job_id VARCHAR(100), 
	pricing_model VARCHAR(10) NOT NULL, 
	total_pre_vat NUMERIC(18, 0), 
	total_vat NUMERIC(18, 0), 
	payment_mode VARCHAR(10), 
	payment_cycle_months INTEGER, 
	parent_contract_id INTEGER, 
	renewed_from_id INTEGER, 
	doc_kind VARCHAR(20) NOT NULL, 
	created_at DATETIME NOT NULL, 
	updated_at DATETIME NOT NULL, 
	PRIMARY KEY (id), 
	FOREIGN KEY(customer_id) REFERENCES customers (id), 
	FOREIGN KEY(product_id) REFERENCES products (id), 
	FOREIGN KEY(template_id) REFERENCES contract_templates (id), 
	FOREIGN KEY(parent_contract_id) REFERENCES contracts (id), 
	FOREIGN KEY(renewed_from_id) REFERENCES contracts (id)
);

CREATE TABLE customer_documents (
	id INTEGER NOT NULL, 
	customer_id INTEGER NOT NULL, 
	doc_type VARCHAR(50) NOT NULL, 
	label VARCHAR(200), 
	file_path VARCHAR(500) NOT NULL, 
	file_name VARCHAR(200) NOT NULL, 
	mime_hint VARCHAR(20), 
	uploaded_at DATETIME NOT NULL, 
	PRIMARY KEY (id), 
	FOREIGN KEY(customer_id) REFERENCES customers (id)
);

CREATE TABLE customer_units (
	id INTEGER NOT NULL, 
	customer_id INTEGER NOT NULL, 
	name VARCHAR(300) NOT NULL, 
	unit_type VARCHAR(50) NOT NULL, 
	address VARCHAR(400), 
	notes TEXT, 
	is_active BOOLEAN NOT NULL, 
	include_in_billing BOOLEAN NOT NULL, 
	sort_order INTEGER NOT NULL, 
	created_at DATETIME NOT NULL, 
	updated_at DATETIME NOT NULL, 
	PRIMARY KEY (id), 
	FOREIGN KEY(customer_id) REFERENCES customers (id) ON DELETE CASCADE
);

CREATE TABLE customers (
	id INTEGER NOT NULL, 
	code VARCHAR(50) NOT NULL, 
	legal_name VARCHAR(300) NOT NULL, 
	short_name VARCHAR(100), 
	customer_type VARCHAR(50) NOT NULL, 
	address VARCHAR(400), 
	ward VARCHAR(100), 
	district VARCHAR(100), 
	province VARCHAR(100), 
	tax_code VARCHAR(30), 
	representative_name VARCHAR(150), 
	representative_gender VARCHAR(10), 
	representative_title VARCHAR(100), 
	bank_account VARCHAR(50), 
	bank_name VARCHAR(200), 
	beneficiary_name VARCHAR(200), 
	contact_name VARCHAR(150), 
	phone VARCHAR(30), 
	email VARCHAR(150), 
	is_active BOOLEAN NOT NULL, 
	is_deleted BOOLEAN NOT NULL, 
	deleted_at DATETIME, 
	bill_main_station BOOLEAN NOT NULL, 
	record_count INTEGER, 
	budget_relation_code VARCHAR(50), 
	kcb_code VARCHAR(30), 
	establishment_decision VARCHAR(100), 
	establishment_date DATE, 
	notes TEXT, 
	created_at DATETIME NOT NULL, 
	updated_at DATETIME NOT NULL, 
	PRIMARY KEY (id)
);

CREATE TABLE products (
	id INTEGER NOT NULL, 
	code VARCHAR(50) NOT NULL, 
	name VARCHAR(200) NOT NULL, 
	product_type VARCHAR(80) NOT NULL, 
	default_price NUMERIC(18, 0), 
	default_vat_rate NUMERIC(5, 2), 
	default_duration_months INTEGER, 
	default_template_id INTEGER, 
	contract_number_format VARCHAR(200), 
	is_active BOOLEAN NOT NULL, 
	group_code VARCHAR(50), 
	kind VARCHAR(20) NOT NULL, 
	unit_label VARCHAR(50), 
	price_period_months INTEGER NOT NULL, 
	quantity_source VARCHAR(20) NOT NULL, 
	min_quantity INTEGER, 
	quantity_step INTEGER, 
	account_limit INTEGER, 
	feature_list TEXT, 
	replaced_by_id INTEGER, 
	sort_order INTEGER NOT NULL, 
	created_at DATETIME NOT NULL, 
	updated_at DATETIME NOT NULL, 
	PRIMARY KEY (id), 
	FOREIGN KEY(default_template_id) REFERENCES contract_templates (id), 
	FOREIGN KEY(replaced_by_id) REFERENCES products (id)
);

CREATE TABLE quotation_issuers (
	id INTEGER NOT NULL, 
	name VARCHAR(500) NOT NULL, 
	address VARCHAR(500) NOT NULL, 
	tax_code VARCHAR(500) NOT NULL, 
	representative VARCHAR(500) NOT NULL, 
	title VARCHAR(500) NOT NULL, 
	phone VARCHAR(500) NOT NULL, 
	email VARCHAR(500) NOT NULL, 
	fingerprint VARCHAR(64) NOT NULL, 
	PRIMARY KEY (id), 
	UNIQUE (fingerprint)
);

CREATE INDEX ix_contract_events_contract_id ON contract_events (contract_id);

CREATE INDEX ix_contract_events_id ON contract_events (id);

CREATE INDEX ix_contract_events_occurred_at ON contract_events (occurred_at);

CREATE INDEX ix_contract_items_contract_id ON contract_items (contract_id);

CREATE INDEX ix_contract_items_id ON contract_items (id);

CREATE UNIQUE INDEX ix_contract_templates_code ON contract_templates (code);

CREATE INDEX ix_contract_templates_id ON contract_templates (id);

CREATE INDEX ix_contract_templates_product_group ON contract_templates (product_group);

CREATE INDEX ix_contracts_batch_job_id ON contracts (batch_job_id);

CREATE UNIQUE INDEX ix_contracts_contract_number ON contracts (contract_number);

CREATE INDEX ix_contracts_customer_id ON contracts (customer_id);

CREATE INDEX ix_contracts_id ON contracts (id);

CREATE INDEX ix_contracts_parent_contract_id ON contracts (parent_contract_id);

CREATE INDEX ix_contracts_renewed_from_id ON contracts (renewed_from_id);

CREATE INDEX ix_contracts_status ON contracts (status);

CREATE INDEX ix_customer_documents_customer_id ON customer_documents (customer_id);

CREATE INDEX ix_customer_documents_id ON customer_documents (id);

CREATE INDEX ix_customer_units_customer_id ON customer_units (customer_id);

CREATE INDEX ix_customer_units_id ON customer_units (id);

CREATE UNIQUE INDEX ix_customers_code ON customers (code);

CREATE INDEX ix_customers_id ON customers (id);

CREATE UNIQUE INDEX ix_products_code ON products (code);

CREATE INDEX ix_products_group_code ON products (group_code);

CREATE INDEX ix_products_id ON products (id);
