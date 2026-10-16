fn main() {
    match sov_treasury::cli::run(std::env::args().skip(1).collect()) {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!(
                "The treasury request could not be completed ({}).",
                sov_treasury::treasury::code(&error)
            );
            std::process::exit(1);
        }
    }
}
