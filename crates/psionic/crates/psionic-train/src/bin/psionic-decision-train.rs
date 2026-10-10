//! `psionic-decision-train` — head-only decision training (roadmap X2a,
//! openagents#11217). See `psionic_train::decision_train`.
//!
//! ```text
//! psionic-decision-train fit --data DIR --out DIR [--recipe FILE] [--seed N]
//!     [--projection R] [--clef-logit] [--hidden-rows-only] [--hidden N] [--epochs N]
//! psionic-decision-train gradcheck
//! ```

use std::path::PathBuf;
use std::process::exit;

use psionic_train::decision_train::{Dataset, Recipe, fit, gradient_check, recipe_from, write_outputs};

fn usage() -> ! {
    eprintln!(
        "usage:\n  psionic-decision-train fit --data DIR --out DIR [--recipe FILE] [--seed N] [--projection R] \
         [--clef-logit] [--hidden-rows-only] [--hidden N] [--epochs N]\n  psionic-decision-train gradcheck"
    );
    exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(verb) = args.next() else { usage() };
    match verb.as_str() {
        "gradcheck" => {
            let worst = (1..=5).map(gradient_check).fold(0f64, f64::max);
            println!("max |analytic - central difference| over 5 seeds: {worst:.3e}");
            if worst > 1e-4 {
                exit(1);
            }
        }
        "fit" => {
            let (mut data, mut out, mut recipe_path) = (None, None, None);
            let mut over: Vec<(String, String)> = Vec::new();
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--data" => data = args.next().map(PathBuf::from),
                    "--out" => out = args.next().map(PathBuf::from),
                    "--recipe" => recipe_path = args.next().map(PathBuf::from),
                    "--clef-logit" | "--hidden-rows-only" => over.push((flag, String::new())),
                    "--seed" | "--projection" | "--hidden" | "--epochs" => {
                        let Some(v) = args.next() else { usage() };
                        over.push((flag, v));
                    }
                    _ => usage(),
                }
            }
            let (Some(data), Some(out)) = (data, out) else { usage() };
            let mut recipe = recipe_from(recipe_path.as_deref(), Recipe::default()).unwrap_or_else(|e| {
                eprintln!("recipe: {e}");
                exit(1)
            });
            for (flag, v) in over {
                let n = || v.parse::<u64>().unwrap_or_else(|_| usage());
                match flag.as_str() {
                    "--clef-logit" => recipe.clef_logit = true,
                    "--hidden-rows-only" => recipe.hidden_rows_only = true,
                    "--seed" => recipe.seed = n(),
                    "--projection" => recipe.projection = n() as usize,
                    "--hidden" => recipe.hidden = n() as usize,
                    "--epochs" => recipe.epochs = n() as usize,
                    _ => usage(),
                }
            }
            let data = Dataset::load(&data).unwrap_or_else(|e| {
                eprintln!("data: {e}");
                exit(1)
            });
            eprintln!(
                "{} rows, {} features, hidden width {}; recipe {}",
                data.rows.len(),
                data.features.len(),
                data.hidden_width,
                recipe.digest()
            );
            let (model, receipt) = fit(&data, &recipe).unwrap_or_else(|e| {
                eprintln!("fit: {e}");
                exit(1)
            });
            for e in &receipt.loss_series {
                eprintln!("epoch {:>3}  train {:.5}  holdout {:.5}", e.epoch, e.train_loss, e.holdout_loss);
            }
            write_outputs(&out, &data, &model, &receipt).unwrap_or_else(|e| {
                eprintln!("write: {e}");
                exit(1)
            });
            println!(
                "best epoch {} of {}; {} train rows ({} issues); {:.1}s; receipt {}",
                receipt.best_epoch,
                receipt.loss_series.len(),
                receipt.train_rows,
                receipt.train_issues,
                receipt.wall_seconds,
                out.join("receipt.json").display()
            );
        }
        _ => usage(),
    }
}
