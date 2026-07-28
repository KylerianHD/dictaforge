mod audio;
mod config;
mod daemon;
mod dbus;
mod hotkey;
mod inject;
mod keymap;
mod stt;
mod tray;
mod vad;

/// Notify (so the failure is visible outside the terminal) and exit.
fn fatal(summary: &str, body: &str) -> ! {
    daemon::notify(summary, body);
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match &args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        // debug aid, kept permanently: record 5 s from the default mic and
        // dump the normalized 16 kHz mono take as a wav for listening
        ["--dump-wav", path] => {
            let stream = audio::Stream::open(None).expect("audio capture");
            let tap = stream.tap();
            let mark = tap.now();
            eprintln!("recording 5 s, speak now");
            std::thread::sleep(std::time::Duration::from_secs(5));
            let samples = tap.take_since(mark).expect("resample");
            audio::write_wav(std::path::Path::new(path), &samples).expect("write wav");
            eprintln!("wrote {} samples (16 kHz mono) to {path}", samples.len());
        }
        // D-Bus surface over the stub pipeline: no audio, no injection,
        // no hotkey, no tray; InjectText prints to stdout
        ["--no-hardware"] => {
            let cfg = config::Config::load(&config::Config::default_path()).expect("config");
            daemon::run(cfg, daemon::Pipeline::stub(), false).expect("dbus service");
        }
        [] => {
            let cfg = match config::Config::load(&config::Config::default_path()) {
                Ok(cfg) => cfg,
                Err(e) => fatal("Broken config", &e.to_string()),
            };
            let spec = keymap::detect(cfg.layout_override.as_ref());
            let injector =
                match inject::InjectorManager::pick(cfg.backend_override.as_deref(), &spec) {
                    Ok(injector) => injector,
                    Err(e) => fatal("No injection backend", &e.to_string()),
                };
            eprintln!("dictaforged: layout {spec:?}, backend {}", injector.name());
            let layout = if spec.variant.is_empty() {
                spec.layout.clone()
            } else {
                format!("{}({})", spec.layout, spec.variant)
            };
            let pipeline = daemon::Pipeline::real(&cfg, layout, injector);
            if let Err(e) = daemon::run(cfg, pipeline, true) {
                fatal("Daemon failed", &e.to_string());
            }
        }
        _ => {
            eprintln!("usage: dictaforged [--no-hardware | --dump-wav <path>]");
            std::process::exit(2);
        }
    }
}
