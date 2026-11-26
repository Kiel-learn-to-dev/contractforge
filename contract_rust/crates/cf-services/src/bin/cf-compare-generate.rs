use std::{env, path::PathBuf};

use cf_db::{clock::SystemClock, paths::DataPaths, store::Store};
use cf_services::{Ctx, contracts::generate::generate_docx};
use serde_json::{Map, Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let root = PathBuf::from(
        args.next()
            .ok_or("usage: cf-compare-generate <data-root> <contract-id>...")?,
    );
    let ids = args
        .map(|arg| arg.parse::<i64>())
        .collect::<Result<Vec<_>, _>>()?;
    if ids.is_empty() {
        return Err("at least one contract id is required".into());
    }

    let paths = DataPaths::new(&root);
    paths.ensure_dirs()?;
    let store = Store::load(&paths.db)?;
    let mut ctx = Ctx::new(store, paths, Box::new(SystemClock));
    let mut outputs = Map::new();

    for id in ids {
        let result = generate_docx(&mut ctx, id, &[])?;
        outputs.insert(
            id.to_string(),
            json!({ "file_name": result.file_name, "file_path": result.file_path }),
        );
    }
    println!("{}", Value::Object(outputs));
    Ok(())
}
