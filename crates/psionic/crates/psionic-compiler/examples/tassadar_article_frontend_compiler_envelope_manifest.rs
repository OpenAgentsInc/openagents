use psionic_compiler::{
    tassadar_article_frontend_compiler_envelope_manifest_path,
    write_tassadar_article_frontend_compiler_envelope_manifest,
};

fn main() {
    let path = tassadar_article_frontend_compiler_envelope_manifest_path();
    let manifest = write_tassadar_article_frontend_compiler_envelope_manifest(&path)
        .expect("write article frontend/compiler envelope manifest");
    println!("wrote {} ({})", path.display(), manifest.manifest_digest);
}
