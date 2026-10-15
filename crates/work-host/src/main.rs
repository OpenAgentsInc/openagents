fn main() {
    match work_host::cli::run(std::env::args().skip(1).collect()) {
        Ok(value) => {
            println!("{value}");
            if matches!(
                value["status"].as_str(),
                Some("conflict" | "refused" | "unknown")
            ) {
                std::process::exit(1);
            }
        }
        Err(_) => {
            eprintln!("The work request could not be completed.");
            std::process::exit(1);
        }
    }
}
