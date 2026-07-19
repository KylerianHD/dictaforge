//! Thin D-Bus client for dictaforged. Hand-rolled args, no clap.

use std::process::ExitCode;

use zbus::blocking::Connection;
use zbus::proxy;

#[proxy(
    interface = "org.dictaforge.Daemon1",
    default_service = "org.dictaforge.Daemon1",
    default_path = "/org/dictaforge/Daemon1"
)]
trait Daemon1 {
    fn toggle(&self) -> zbus::Result<()>;
    fn start(&self) -> zbus::Result<()>;
    fn stop(&self) -> zbus::Result<()>;
    fn status(&self) -> zbus::Result<String>;
    fn inject_text(&self, text: &str) -> zbus::Result<()>;
}

const USAGE: &str = "usage: dictaforge-cli toggle|start|stop|status|type <text>";

fn run(args: &[&str]) -> zbus::Result<()> {
    let conn = Connection::session()?;
    let daemon = Daemon1ProxyBlocking::new(&conn)?;
    match args {
        ["toggle"] => daemon.toggle(),
        ["start"] => daemon.start(),
        ["stop"] => daemon.stop(),
        ["status"] => {
            println!("{}", daemon.status()?);
            Ok(())
        }
        ["type", text] => daemon.inject_text(text),
        _ => unreachable!("run() only sees validated args"),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args[..] {
        ["toggle"] | ["start"] | ["stop"] | ["status"] | ["type", _] => {}
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    }
    if let Err(e) = run(&args) {
        eprintln!("dictaforge-cli: {e} (is dictaforged running?)");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
