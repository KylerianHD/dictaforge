// ponytail: allows drop when the daemon (Task 10) consumes everything
#[allow(dead_code, unused_imports)]
mod audio;
#[allow(dead_code, unused_imports)]
mod inject;
#[allow(dead_code, unused_imports)]
mod keymap;

use inject::Injector;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // temporary manual-acceptance flag, removed in Task 10 when
    // dictaforge-cli type covers it
    if let ["--inject-test-xtest", text] = &args.iter().map(String::as_str).collect::<Vec<_>>()[..]
    {
        let mut injector = inject::XtestInjector::new();
        injector.probe().expect("xtest available");
        eprintln!("focus the target field, injecting in 3 s");
        std::thread::sleep(std::time::Duration::from_secs(3));
        injector.inject(text).expect("inject");
        return;
    }
    if let ["--inject-test-vk", text] = &args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        let mut injector = inject::VirtualKeyboardInjector::new();
        injector.probe().expect("virtual keyboard available");
        eprintln!("focus the target field, injecting in 3 s");
        std::thread::sleep(std::time::Duration::from_secs(3));
        injector.inject(text).expect("inject");
        return;
    }
    if let ["--inject-test", text] = &args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        let spec = keymap::detect(None);
        eprintln!("layout: {spec:?}");
        let index = keymap::KeymapIndex::from_spec(&spec).expect("keymap compiles");
        let mut injector = inject::UinputInjector::new(index);
        injector.probe().expect("uinput available");
        eprintln!("focus the target field, injecting in 3 s");
        std::thread::sleep(std::time::Duration::from_secs(3));
        injector.inject(text).expect("inject");
        return;
    }
    // debug aid, kept permanently: record 5 s from the default mic and dump
    // the normalized 16 kHz mono take as a wav for listening
    if let ["--dump-wav", path] = &args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        let handle = audio::Recorder::start(None).expect("audio capture");
        eprintln!("recording 5 s, speak now");
        std::thread::sleep(std::time::Duration::from_secs(5));
        let mut samples = handle.stop().expect("resample");
        audio::normalize(&mut samples);
        audio::write_wav(std::path::Path::new(path), &samples).expect("write wav");
        eprintln!("wrote {} samples (16 kHz mono) to {path}", samples.len());
        return;
    }
    // ponytail: stub until M1 wires up audio, STT, and injection
    println!(
        "dictaforged {} (nothing implemented yet)",
        env!("CARGO_PKG_VERSION")
    );
}
