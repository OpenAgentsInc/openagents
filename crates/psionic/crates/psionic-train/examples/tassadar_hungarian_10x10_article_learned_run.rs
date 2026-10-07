use std::{env, path::PathBuf, process::ExitCode};

use psionic_train::{
    TASSADAR_EXECUTOR_HUNGARIAN_10X10_ARTICLE_OUTPUT_DIR,
    augment_tassadar_hungarian_10x10_article_run_with_review,
    execute_tassadar_hungarian_10x10_article_run,
};

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1).collect::<Vec<_>>();
    let augment_only = args
        .first()
        .is_some_and(|argument| argument == "--augment-only");
    if augment_only {
        args.remove(0);
    }
    let output_dir = args
        .into_iter()
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TASSADAR_EXECUTOR_HUNGARIAN_10X10_ARTICLE_OUTPUT_DIR));

    let result = if augment_only {
        augment_tassadar_hungarian_10x10_article_run_with_review(&output_dir)
    } else {
        execute_tassadar_hungarian_10x10_article_run(&output_dir)
    };

    match result {
        Ok(bundle) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&bundle)
                    .expect("tassadar article learned run bundle should serialize"),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to {} Tassadar Hungarian-10x10 article learned run `{}`: {error}",
                if augment_only { "augment" } else { "execute" },
                output_dir.display()
            );
            ExitCode::FAILURE
        }
    }
}
