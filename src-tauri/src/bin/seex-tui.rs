fn main() {
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        match flag.as_str() {
            "-h" | "--help" => {
                println!("seex-tui");
                println!();
                println!("Interactive ratatui frontend for SeEx.");
                println!();
                println!("Usage:");
                println!("  seex-tui");
                println!("  seex-tui --help");
                println!("  seex-tui --version");
                return;
            }
            "-V" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            _ => {}
        }
    }

    if let Err(err) = seex_lib::tui::run() {
        eprintln!("seex-tui failed: {err}");
        std::process::exit(1);
    }
}
