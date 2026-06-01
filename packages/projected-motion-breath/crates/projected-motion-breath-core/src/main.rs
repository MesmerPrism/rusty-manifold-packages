use projected_motion_breath_core::{run_controller_preflight, validate_package_goldens};
use std::env;
use std::path::PathBuf;

fn main() {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage_and_exit();
    };

    let Some(flag) = args.next() else {
        print_usage_and_exit();
    };
    if flag != "--package-root" {
        print_usage_and_exit();
    }

    let Some(package_root) = args.next() else {
        print_usage_and_exit();
    };
    if args.next().is_some() {
        print_usage_and_exit();
    }

    let package_root = PathBuf::from(package_root);
    let output = match command.as_str() {
        "validate-goldens" => validate_package_goldens(package_root)
            .map(|report| (report.status.clone(), serde_json::to_string(&report))),
        "controller-preflight" => run_controller_preflight(package_root)
            .map(|report| (report.status.clone(), serde_json::to_string(&report))),
        _ => print_usage_and_exit(),
    };

    match output {
        Ok((status, report)) => {
            println!("{}", report.expect("validation report serializes"));
            if status != "pass" {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn print_usage_and_exit() -> ! {
    eprintln!(
        "usage: projected-motion-breath-core <validate-goldens|controller-preflight> --package-root <package-root>"
    );
    std::process::exit(2);
}
