use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=openapi-minimal.json");

    let src = Path::new("openapi-minimal.json");
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_file = Path::new(&out_dir).join("codegen.rs");

    let file = File::open(src).expect("Failed to open openapi-minimal.json");
    let spec: openapiv3::OpenAPI =
        serde_json::from_reader(file).expect("Failed to parse openapi-minimal.json");

    let mut generator = progenitor::Generator::default();

    let tokens = generator
        .generate_tokens(&spec)
        .expect("Failed to generate code from OpenAPI spec");

    let ast = syn::parse2(tokens).expect("Failed to parse generated tokens");
    let content = prettyplease::unparse(&ast);
    let content = content.replace(
        "elided_named_lifetimes",
        "mismatched_lifetime_syntaxes",
    );

    std::fs::write(&out_file, content).expect("Failed to write generated code");
}
