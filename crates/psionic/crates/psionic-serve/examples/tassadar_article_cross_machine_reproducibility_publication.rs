use psionic_serve::{
    tassadar_article_cross_machine_reproducibility_publication_path,
    write_tassadar_article_cross_machine_reproducibility_publication,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let publication_path = tassadar_article_cross_machine_reproducibility_publication_path();
    let publication =
        write_tassadar_article_cross_machine_reproducibility_publication(&publication_path)?;
    println!(
        "wrote {} ({})",
        publication_path.display(),
        publication.publication_digest
    );
    println!(
        "supported_machine_classes={} selected_decode_mode={}",
        publication.supported_machine_class_ids.len(),
        publication.selected_decode_mode.as_str(),
    );
    Ok(())
}
