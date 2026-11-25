use std::{env, fs, path::PathBuf};

use cf_docx::{RenderData, Template, write_new_file};

fn run() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let invalid =
        |message: &'static str| std::io::Error::new(std::io::ErrorKind::InvalidInput, message);
    let template_path = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| invalid("Cách dùng: cf-render <mẫu.docx> <data.json> <thư_mục_ra> [tên]"))?;
    let data_path = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| invalid("Thiếu data.json"))?;
    let output_dir = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| invalid("Thiếu thư mục đầu ra"))?;
    let file_name = args
        .next()
        .and_then(|value| value.into_string().ok())
        .unwrap_or_else(|| "rendered.docx".into());
    let template = Template::compile(&fs::read(template_path)?)?;
    let data: RenderData = serde_json::from_slice(&fs::read(data_path)?)?;
    let bytes = template.render(&data)?;
    let (path, _) = write_new_file(&output_dir, &file_name, &bytes)?;
    println!("{}", path.display());
    Ok(path)
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
