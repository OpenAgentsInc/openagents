//! Run the native iOS shell or export the same public fixture on the host.

fn main() {
    #[cfg(target_os = "ios")]
    coder_mobile_probe::ios::run();

    #[cfg(not(target_os = "ios"))]
    {
        let steps = coder_mobile_probe::fixture();
        match std::env::args().nth(1).as_deref() {
            Some("--html") => println!("{}", coder_mobile_probe::html(&steps)),
            Some("--atif") => println!("{}", coder_mobile_probe::atif_document(&steps)),
            None | Some("--text") => print!("{}", coder_mobile_probe::transcript(&steps)),
            _ => {
                eprintln!("usage: coder-mobile-probe [--text|--html|--atif]");
                std::process::exit(64);
            }
        }
    }
}
