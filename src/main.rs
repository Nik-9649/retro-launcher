use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if let Err(error) = retro_launcher::run_cli(args) {
        eprintln!("retro-launcher: error: {:#}", error);
        process::exit(1);
    }
}
