//! The outbound Lab worker program.
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = if args.len() == 1 && args[0] == "--echo-handler" {
        lab_jobs::worker::echo_handler()
    } else if (args.len() == 2 || (args.len() == 3 && args[2] == "--once")) && args[0] == "--config"
    {
        match lab_jobs::worker::Worker::open(std::path::Path::new(&args[1])) {
            Ok(worker) => loop {
                match worker.run_once().await {
                    Ok(_) if args.len() == 3 => break Ok(()),
                    Ok(_) => (),
                    Err(error) => break Err(error),
                }
            },
            Err(error) => Err(error),
        }
    } else {
        Err(lab_jobs::worker::Error(
            "Run labs-worker --config /absolute/path/worker.json [--once].",
        ))
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
