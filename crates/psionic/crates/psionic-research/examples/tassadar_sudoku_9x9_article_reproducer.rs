use psionic_research::{
    tassadar_sudoku_9x9_article_reproducer_report_path,
    write_tassadar_sudoku_9x9_article_reproducer_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report =
        write_tassadar_sudoku_9x9_article_reproducer_report(
            tassadar_sudoku_9x9_article_reproducer_report_path(),
        )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
