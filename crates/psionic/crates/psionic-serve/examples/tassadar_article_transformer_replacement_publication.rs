use std::{fs, path::Path};

use psionic_models::TassadarExecutorFixture;
use psionic_serve::{
    LocalTassadarExecutorService, TASSADAR_ARTICLE_TRANSFORMER_REPLACEMENT_PUBLICATION_REF,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let publication = LocalTassadarExecutorService::new()
        .with_fixture(TassadarExecutorFixture::article_i32_compute_v1())
        .article_transformer_replacement_publication(Some(
            TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
        ))?;

    let path = Path::new(TASSADAR_ARTICLE_TRANSFORMER_REPLACEMENT_PUBLICATION_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&publication)?),
    )?;
    println!(
        "wrote {} with digest {}",
        TASSADAR_ARTICLE_TRANSFORMER_REPLACEMENT_PUBLICATION_REF, publication.publication_digest
    );
    Ok(())
}
