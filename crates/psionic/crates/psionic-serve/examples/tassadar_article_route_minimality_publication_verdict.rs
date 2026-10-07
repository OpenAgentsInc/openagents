use psionic_serve::{
    tassadar_article_route_minimality_publication_verdict_path,
    write_tassadar_article_route_minimality_publication_verdict,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let publication_path = tassadar_article_route_minimality_publication_verdict_path();
    let publication =
        write_tassadar_article_route_minimality_publication_verdict(&publication_path)?;
    println!(
        "wrote {} ({})",
        publication_path.display(),
        publication.publication_digest
    );
    println!(
        "public_posture={:?} route_minimality_audit_green={}",
        publication.public_posture, publication.route_minimality_audit_green,
    );
    Ok(())
}
