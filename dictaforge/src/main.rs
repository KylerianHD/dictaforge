mod overlay;

const USAGE: &str = "usage: dictaforge --overlay";

fn main() -> std::process::ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--overlay") => {
            if overlay::run() == gtk4::glib::ExitCode::SUCCESS {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::FAILURE
            }
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            std::process::ExitCode::SUCCESS
        }
        // ponytail: the settings window is Task 4; until then this binary
        // exists only to carry the overlay
        None => {
            println!(
                "dictaforge {} (settings GUI not built yet)\n{USAGE}",
                env!("CARGO_PKG_VERSION")
            );
            std::process::ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("dictaforge: unknown argument {other:?}\n{USAGE}");
            std::process::ExitCode::from(2)
        }
    }
}
