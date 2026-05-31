use polar_h10_core::{run_graph, validate_goldens, GraphManifest, RuntimeInput};
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;
    match command.as_str() {
        "validate-goldens" => {
            let package_root = optional_path_arg(&mut args, "--package-root")?
                .unwrap_or_else(default_package_root);
            match validate_goldens(&package_root) {
                Ok(()) => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "$schema": "rusty.manifold.polar_h10.core_validation_report.v1",
                            "status": "pass",
                            "package_root": package_root
                        })
                    );
                    Ok(())
                }
                Err(errors) => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "$schema": "rusty.manifold.polar_h10.core_validation_report.v1",
                            "status": "fail",
                            "package_root": package_root,
                            "errors": errors
                        })
                    );
                    Err("golden validation failed".to_string())
                }
            }
        }
        "run-fixture" => run_fixture(args.collect()),
        _ => Err(usage()),
    }
}

fn run_fixture(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter();
    let mut graph_path = None;
    let mut input_path = None;
    let mut out_path = None;
    let mut selected_modules = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--graph" => graph_path = Some(next_path(&mut args, "--graph")?),
            "--input" => input_path = Some(next_path(&mut args, "--input")?),
            "--out" => out_path = Some(next_path(&mut args, "--out")?),
            "--select" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--select requires a value".to_string())?;
                for item in value.split(',') {
                    let item = item.trim();
                    if !item.is_empty() {
                        selected_modules.push(item.to_string());
                    }
                }
            }
            _ => return Err(format!("unknown argument: {arg}\n{}", usage())),
        }
    }
    if selected_modules.is_empty() {
        return Err("run-fixture requires at least one --select module".to_string());
    }
    let graph_path = graph_path.ok_or_else(|| "run-fixture requires --graph".to_string())?;
    let input_path = input_path.ok_or_else(|| "run-fixture requires --input".to_string())?;
    let graph: GraphManifest = read_json(&graph_path)?;
    let input: RuntimeInput = read_json(&input_path)?;
    let report = run_graph(&graph, &input, &selected_modules);
    let text = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    if let Some(out_path) = out_path {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(out_path, &text).map_err(|error| error.to_string())?;
    }
    println!("{text}");
    if report.status == "pass" {
        Ok(())
    } else {
        Err("graph fixture run failed".to_string())
    }
}

fn optional_path_arg<I>(args: &mut I, name: &str) -> Result<Option<PathBuf>, String>
where
    I: Iterator<Item = String>,
{
    let mut path = None;
    while let Some(arg) = args.next() {
        if arg == name {
            path = Some(next_path(args, name)?);
        } else {
            return Err(format!("unknown argument: {arg}\n{}", usage()));
        }
    }
    Ok(path)
}

fn next_path<I>(args: &mut I, name: &str) -> Result<PathBuf, String>
where
    I: Iterator<Item = String>,
{
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("{name} requires a path"))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<T, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}:{error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{}:{error}", path.display()))
}

fn default_package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate directory has parent")
        .parent()
        .expect("crates directory has parent")
        .to_path_buf()
}

fn usage() -> String {
    "usage: polar-h10-core validate-goldens [--package-root <path>] | run-fixture --graph <path> --input <path> --select <module>[,<module>] [--out <path>]".to_string()
}
