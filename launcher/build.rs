//! Embeds Wisp's icon (`packaging/icons/wisp.ico`) in the Windows executables, as icon
//! resource 1: Explorer and the task bar show it, and the window takes it as its icon.
fn main() {
    println!("cargo:rerun-if-env-changed=WISP_MSA_CLIENT_ID");
    let dir = wisp_compat::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let ico = format!("{dir}/../packaging/icons/wisp.ico");
    println!("cargo:rerun-if-changed={ico}");
    #[cfg(windows)]
    if wisp_compat::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(&ico);
        resource.set("ProductName", "Wisp Client");
        resource.set("FileDescription", "Wisp Client");
        resource.set("CompanyName", "Lyvt-Dev");
        if let Err(e) = resource.compile() {
            panic!("embedding {ico}: {e}");
        }
    }
}
