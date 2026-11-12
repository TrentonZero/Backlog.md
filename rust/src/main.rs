fn main() {
    if let Err(error) = backlog_md::cli::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
