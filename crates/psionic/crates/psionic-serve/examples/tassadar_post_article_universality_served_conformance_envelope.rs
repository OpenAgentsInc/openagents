use psionic_serve::{
    tassadar_post_article_universality_served_conformance_envelope_path,
    write_tassadar_post_article_universality_served_conformance_envelope,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let publication_path = tassadar_post_article_universality_served_conformance_envelope_path();
    let publication =
        write_tassadar_post_article_universality_served_conformance_envelope(&publication_path)?;
    println!(
        "wrote {} ({})",
        publication_path.display(),
        publication.publication_digest
    );
    println!(
        "served_suppression_boundary_preserved={} served_public_universality_allowed={}",
        publication.served_suppression_boundary_preserved,
        publication.served_public_universality_allowed,
    );
    Ok(())
}
