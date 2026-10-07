use psionic_runtime::{
    tassadar_article_hard_sudoku_benchmark_root_path,
    write_tassadar_article_hard_sudoku_benchmark_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bundle = write_tassadar_article_hard_sudoku_benchmark_bundle(
        tassadar_article_hard_sudoku_benchmark_root_path(),
    )?;
    println!(
        "wrote {} ({})",
        tassadar_article_hard_sudoku_benchmark_root_path()
            .join("article_hard_sudoku_benchmark_bundle.json")
            .display(),
        bundle.bundle_digest
    );
    Ok(())
}
