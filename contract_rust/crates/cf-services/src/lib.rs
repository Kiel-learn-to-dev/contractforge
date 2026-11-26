//! Ca sử dụng của ứng dụng ContractForge: nghiệp vụ gọi Store, không đụng Slint.

pub mod addenda;
pub mod assets;
pub mod batch;
pub mod contracts;
pub mod ctx;
pub mod customers;
pub mod dashboard;
pub mod dossier;
pub mod error;
pub mod events;
pub mod products;
pub mod quotations;
pub mod settings;
pub mod templates;

pub use ctx::Ctx;
pub use error::ServiceError;
