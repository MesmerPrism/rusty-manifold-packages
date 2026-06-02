use projected_motion_breath_core::{
    run_controller_preflight, run_live_route_from_broker_events, run_live_route_self_test,
    validate_package_goldens,
};
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
    let package_root = PathBuf::from(package_root);
    let remaining_args: Vec<String> = args.collect();
    let output = match command.as_str() {
        "validate-goldens" => {
            if !remaining_args.is_empty() {
                print_usage_and_exit();
            }
            validate_package_goldens(package_root)
                .map(|report| (report.status.clone(), serde_json::to_string(&report)))
        }
        "controller-preflight" => {
            if !remaining_args.is_empty() {
                print_usage_and_exit();
            }
            run_controller_preflight(package_root)
                .map(|report| (report.status.clone(), serde_json::to_string(&report)))
        }
        "live-route-self-test" => {
            if !remaining_args.is_empty() {
                print_usage_and_exit();
            }
            run_live_route_self_test(package_root)
                .map(|report| (report.status.clone(), serde_json::to_string(&report)))
        }
        "live-route-from-events" => {
            if remaining_args.len() != 2 {
                print_usage_and_exit();
            }
            if remaining_args[0] != "--events-jsonl" {
                print_usage_and_exit();
            }
            run_live_route_from_broker_events(package_root, PathBuf::from(&remaining_args[1]))
                .map(|report| (report.status.clone(), serde_json::to_string(&report)))
        }
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
        "usage: projected-motion-breath-core <validate-goldens|controller-preflight|live-route-self-test> --package-root <package-root>\n       projected-motion-breath-core live-route-from-events --package-root <package-root> --events-jsonl <events.jsonl>"
    );
    std::process::exit(2);
}
