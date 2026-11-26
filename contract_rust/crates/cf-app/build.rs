fn main() {
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=contractforge.rc");
    println!("cargo:rerun-if-changed=../../../cf_icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("contractforge.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("Không nhúng được icon ContractForge");
    }
    let config = slint_build::CompilerConfiguration::new()
        .with_edition((1, 19))
        .with_style("fluent".into());
    slint_build::compile_with_config("ui/app.slint", config)
        .unwrap_or_else(|error| panic!("Không biên dịch được giao diện: {error}"));
}
