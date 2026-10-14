//! Regenerate the checked MuJoCo 3.3.7 C layout; not a product runtime.
fn main() {
    let mut args = std::env::args().skip(1);
    let headers = args.next().expect("Give the MuJoCo 3.3.7 include folder");
    let output = args.next().expect("Give the output Rust file");
    let mut builder = bindgen::Builder::default()
        .header(format!("{headers}/mujoco/mujoco.h"))
        .clang_arg(format!("-I{headers}"))
        .allowlist_type("mjModel_|mjData_|mjContact_|mjOption_|mjStatistic_|mjVFS_")
        .allowlist_function("mj_loadXML|mj_makeData|mj_deleteModel|mj_deleteData|mj_step|mj_resetData|mj_forward|mj_getState|mj_setState|mj_stateSize|mj_name2id|mj_contactForce|mj_objectVelocity|mj_version|mj_saveLastXML")
        .allowlist_var("mjVERSION_HEADER")
        .derive_debug(false)
        .generate_comments(false)
        .layout_tests(false);
    // Explicit additional Clang options support SDK/sysroot include paths.
    for argument in args {
        builder = builder.clang_arg(argument);
    }
    let generated = builder
        .generate()
        .expect("Generate the MuJoCo 3.3.7 bindings");
    assert!(
        generated
            .to_string()
            .contains("mjVERSION_HEADER: u32 = 337")
    );
    std::fs::write(
        output,
        generated
            .to_string()
            .replace("extern \"C\" {", "unsafe extern \"C\" {"),
    )
    .expect("Write the bindings");
}
