use psionic_serve::{
    tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication_path,
    write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let publication_path =
        tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication_path();
    let publication =
        write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication(
            &publication_path,
        )?;
    println!(
        "wrote {} ({})",
        publication_path.display(),
        publication.publication_digest
    );
    println!(
        "blocked_by={} served_plugin_surface_ids={}",
        publication.blocked_by.len(),
        publication.served_plugin_surface_ids.len(),
    );
    Ok(())
}
