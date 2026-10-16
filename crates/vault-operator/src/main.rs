fn main() {
    match vault_operator::cli::run(std::env::args().skip(1).collect()) {
        Ok(value) => {
            println!("{value}");
            if matches!(
                value["status"].as_str(),
                Some("refused" | "unknown" | "failed")
            ) {
                std::process::exit(1);
            }
        }
        Err(error) => {
            let code = error.split(':').next().unwrap_or("unavailable");
            eprintln!("The vault operator request could not be completed ({code}).");
            std::process::exit(1);
        }
    }
}
