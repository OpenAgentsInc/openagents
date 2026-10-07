use psionic_train::{serve_qwen_legal_pylon_tcp_worker_once, PylonLocalWorkerRunOptions};

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!(
            "usage: cargo run -p psionic-train --bin qwen_legal_pylon_worker_server -- <bind-addr> <worker-id>"
        );
        std::process::exit(2);
    }
    let options = PylonLocalWorkerRunOptions {
        worker_id: args[2].clone(),
        started_at_ms: 100_000,
        emit_outputs: true,
    };
    if let Err(error) = serve_qwen_legal_pylon_tcp_worker_once(args[1].as_str(), options) {
        eprintln!("qwen legal pylon worker server failed: {error}");
        std::process::exit(1);
    }
}
