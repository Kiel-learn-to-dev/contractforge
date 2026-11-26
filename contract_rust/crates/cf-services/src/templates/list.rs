use cf_core::template_rules::{self, Role};
use cf_db::{model::ContractTemplate, store::Store};

pub fn list_templates(store: &Store, active_only: bool) -> Vec<ContractTemplate> {
    let mut rows = store
        .templates
        .values()
        .filter(|template| !active_only || template.is_active)
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        a.contract_type
            .cmp(&b.contract_type)
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

pub fn templates_for<'a>(
    store: &'a Store,
    group: Option<&str>,
    role: Role,
    active_only: bool,
) -> Vec<&'a ContractTemplate> {
    let group = template_rules::normalize_group(group);
    let mut rows = store
        .templates
        .values()
        .filter(|template| {
            (!active_only || template.is_active)
                && template_rules::has_role(&template.contract_type, role)
                && template_rules::normalize_group(template.product_group.as_deref())
                    .is_none_or(|template_group| Some(template_group) == group)
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|template| {
        (
            template_rules::normalize_group(template.product_group.as_deref()).is_none(),
            template.name.to_lowercase(),
        )
    });
    rows
}

pub fn default_template_for(
    store: &Store,
    group: Option<&str>,
    role: Role,
) -> Option<ContractTemplate> {
    let group = template_rules::normalize_group(group)?;
    let own = templates_for(store, Some(&group), role, true)
        .into_iter()
        .filter(|template| {
            template_rules::normalize_group(template.product_group.as_deref()).as_ref()
                == Some(&group)
        })
        .collect::<Vec<_>>();
    (own.len() == 1).then(|| own[0].clone())
}
