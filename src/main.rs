fn main() {
    let args: Vec<String> = std::env::args().collect();
    let use_cli = args.iter().any(|arg| arg == "--cli");

    if use_cli {
        let cli_args: Vec<String> = std::iter::once(args[0].clone())
            .chain(args.into_iter().skip(1).filter(|arg| arg != "--cli"))
            .collect();

        let status = match bitorrentclient::cli::run_from(cli_args) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("Error: {err:#}");
                1
            }
        };
        std::process::exit(status);
    }

    if let Err(err) = bitorrentclient::gui::run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}
