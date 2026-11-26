//! Read-only comparison of ordinary model reads and a registered pack overlay.
use std::{path::PathBuf, time::Instant};
use wisp_core::prepared_files;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let base = PathBuf::from(args.next().ok_or("expected prepared root")?);
    let overlay = PathBuf::from(args.next().ok_or("expected scratch overlay root")?);
    let archive = prepared_files::open(&base).ok_or("prepared archive is not usable")?;
    let names: Vec<String> = archive
        .list("assets/minecraft/models")
        .unwrap_or_default()
        .into_iter()
        .take(1024)
        .map(str::to_owned)
        .collect();
    let start = Instant::now();
    let expected: Vec<Vec<u8>> = names
        .iter()
        .map(|name| std::fs::read(base.join(name)))
        .collect::<Result<_, _>>()?;
    let disk = start.elapsed();
    prepared_files::register_overlay(&overlay, &base, &[], &[])?;
    let start = Instant::now();
    for (name, expected) in names.iter().zip(expected) {
        assert_eq!(prepared_files::read(&overlay.join(name))?, expected);
    }
    println!(
        "{} models; ordinary files {:.2} ms; indexed overlay {:.2} ms; bytes identical",
        names.len(),
        disk.as_secs_f64() * 1000.,
        start.elapsed().as_secs_f64() * 1000.
    );
    Ok(())
}
