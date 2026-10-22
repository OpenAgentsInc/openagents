#[tokio::main]
async fn main() {
    if let Err(error) =
        kiln_host::workspace::run(&std::env::args().skip(1).collect::<Vec<_>>()).await
    {
        eprintln!("kiln-workspace: {error}");
        std::process::exit(1);
    }
}
