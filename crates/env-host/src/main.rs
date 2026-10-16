fn main() {
    match env_host::cli::run(std::env::args().skip(1).collect()) {
        Ok(value) => {
            println!("{value}");
            if matches!(
                value["status"].as_str(),
                Some("conflict" | "refused" | "unknown")
            ) {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!(
                "The environment request could not be completed ({}).",
                env_host::allocator::code(&error)
            );
            std::process::exit(1);
        }
    }
}
