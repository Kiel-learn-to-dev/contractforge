use std::path::PathBuf;

use cf_core::render_context::PartyB;
use cf_services::{Ctx, settings as settings_service};
use slint::ComponentHandle;

use crate::{
    CustomerHub, MainWindow, SettingsPartyB, files, nav,
    ui_helpers::{ToastKind, toast_from_worker},
    worker::Worker,
};

struct LoadedSettings {
    party_b: PartyB,
    addendum_format: String,
    setup_required: bool,
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>, data_root: PathBuf) {
    let weak = ui.as_weak();
    worker.run(
        |ctx| LoadedSettings {
            party_b: settings_service::get_party_b_settings(ctx.store()),
            addendum_format: settings_service::addendum_format(ctx.store()),
            setup_required: !settings_service::is_organization_profile_complete(ctx.store()),
        },
        move |settings| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            ui.set_settings_party_b(to_slint(settings.party_b));
            ui.set_settings_addendum_format(settings.addendum_format.into());
            ui.set_settings_setup_required(settings.setup_required);
        },
    );

    let party_b_worker = worker.clone();
    let weak_after_save = ui.as_weak();
    ui.on_settings_save_party_b(move |party_b| {
        let values = from_slint(party_b);
        let weak_done = weak_after_save.clone();
        party_b_worker.run(
            move |ctx| {
                settings_service::save_party_b(ctx.store_mut(), values)?;
                Ok::<_, cf_services::ServiceError>(
                    !settings_service::is_organization_profile_complete(ctx.store()),
                )
            },
            move |result| match result {
                Ok(setup_req) => {
                    toast_from_worker(
                        ToastKind::Success,
                        "Đã lưu thông tin Bên B thành công.".into(),
                    );
                    if let Some(ui) = weak_done.upgrade() {
                        ui.set_settings_setup_required(setup_req);
                    }
                }
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            },
        );
    });

    let addendum_worker = worker.clone();
    ui.on_settings_save_addendum_format(move |format| {
        let format = format.to_string();
        addendum_worker.run(
            move |ctx| settings_service::save_addendum_format(ctx.store_mut(), &format),
            |result| match result {
                Ok(()) => toast_from_worker(ToastKind::Success, "Đã lưu khuôn số phụ lục.".into()),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            },
        );
    });

    let weak_trash = ui.as_weak();
    ui.on_settings_open_customer_trash(move || {
        if let Some(ui) = weak_trash.upgrade() {
            ui.invoke_navigate(nav::Screen::Customers.index());
            ui.global::<CustomerHub>().invoke_open_trash();
        }
    });

    ui.on_settings_open_data_root(move || match files::open_path(&data_root) {
        Ok(()) => {}
        Err(error) => toast_from_worker(
            ToastKind::Danger,
            format!("Không mở được thư mục dữ liệu: {error}"),
        ),
    });
}

fn to_slint(party_b: PartyB) -> SettingsPartyB {
    SettingsPartyB {
        name: party_b.name.into(),
        address: party_b.address.into(),
        bank_account: party_b.bank_account.into(),
        bank_name: party_b.bank_name.into(),
        beneficiary: party_b.beneficiary.into(),
        tax_code: party_b.tax_code.into(),
        title: party_b.title.into(),
        representative: party_b.representative.into(),
        authorization_number: party_b.authorization_number.into(),
        authorization_date: party_b.authorization_date.into(),
        phone: party_b.phone.into(),
        contact_name: party_b.contact_name.into(),
        contact_email: party_b.contact_email.into(),
    }
}

fn from_slint(party_b: SettingsPartyB) -> Vec<(String, String)> {
    [
        ("party_b_name", party_b.name),
        ("party_b_address", party_b.address),
        ("party_b_bank_account", party_b.bank_account),
        ("party_b_bank_name", party_b.bank_name),
        ("party_b_beneficiary", party_b.beneficiary),
        ("party_b_tax_code", party_b.tax_code),
        ("party_b_title", party_b.title),
        ("party_b_representative", party_b.representative),
        ("party_b_authorization_number", party_b.authorization_number),
        ("party_b_authorization_date", party_b.authorization_date),
        ("party_b_phone", party_b.phone),
        ("party_b_contact_name", party_b.contact_name),
        ("party_b_contact_email", party_b.contact_email),
    ]
    .into_iter()
    .map(|(key, value)| (key.into(), value.to_string()))
    .collect()
}
