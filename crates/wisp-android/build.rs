//! Phones with 16 KB memory pages (Android 15 and later) only load libraries whose segments are
//! aligned to them; 4 KB phones load those too.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-arg-cdylib=-Wl,-z,max-page-size=16384");
    }
}
