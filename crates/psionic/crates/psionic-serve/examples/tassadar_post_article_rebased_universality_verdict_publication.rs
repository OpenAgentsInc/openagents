use psionic_serve::{
    tassadar_post_article_rebased_universality_verdict_publication_path,
    write_tassadar_post_article_rebased_universality_verdict_publication,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let publication_path = tassadar_post_article_rebased_universality_verdict_publication_path();
    let publication =
        write_tassadar_post_article_rebased_universality_verdict_publication(&publication_path)?;
    println!(
        "wrote {} ({})",
        publication_path.display(),
        publication.publication_digest
    );
    println!(
        "theory_green={} operator_green={} served_green={} rebase_claim_allowed={}",
        publication.theory_green,
        publication.operator_green,
        publication.served_green,
        publication.rebase_claim_allowed,
    );
    Ok(())
}
