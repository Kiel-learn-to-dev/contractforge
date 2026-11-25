fn main() {
    println!("cargo:rerun-if-changed=ui/main.slint");
    println!("cargo:rerun-if-changed=ui/chart.slint");
    let config = slint_build::CompilerConfiguration::new()
        .with_edition((1, 19))
        .with_style("fluent".into());
    slint_build::compile_with_config("ui/main.slint", config)
        .unwrap_or_else(|error| panic!("Không biên dịch được giao diện thử: {error}"));
}
