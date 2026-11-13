fn main() {
    let root =
        std::path::PathBuf::from(wisp_compat::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let out_dir = std::path::PathBuf::from(wisp_compat::var_os("OUT_DIR").unwrap());
    wisp_mod_api_gen::generate(&root, &out_dir, wisp_mod_api_gen::Target::Render)
        .expect("generating native render mod API");
}
