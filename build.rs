fn main() {
    println!("cargo:rustc-env=ASKAMA_FILTERS=markdown,safe");
}
