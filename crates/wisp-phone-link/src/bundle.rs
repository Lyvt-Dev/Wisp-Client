//! The stream of files: a header, then each file with its SHA-256, then an end mark.
//!
//! ```text
//! "WISPBNDL" u32 version  u64 files  u64 bytes
//! files × { u16 path length, path (UTF-8, '/'-separated, relative), u64 size, data, sha256[32] }
//! "WISPEND!"
//! ```
//! Numbers are little-endian. A reader checks every path before it writes anything and every
//! file's hash after, so a damaged stream or a hostile sender can't write outside the folder.
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

const MAGIC: &[u8; 8] = b"WISPBNDL";
const END: &[u8; 8] = b"WISPEND!";
const VERSION: u32 = 1;
/// Longer paths are refused (the prepared files' longest is far shorter).
const MAX_PATH: usize = 1024;

/// The files under `root`, sorted, as '/'-separated relative paths with their sizes.
pub fn list(root: &Path) -> io::Result<Vec<(String, u64)>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(io::Error::other)?
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((relative, entry.metadata()?.len()));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Writes every file of `files` (from [`list`]) under `root` to `out`.
pub fn write(root: &Path, files: &[(String, u64)], out: &mut impl Write) -> io::Result<()> {
    let total: u64 = files.iter().map(|(_, size)| size).sum();
    out.write_all(MAGIC)?;
    out.write_all(&VERSION.to_le_bytes())?;
    out.write_all(&(files.len() as u64).to_le_bytes())?;
    out.write_all(&total.to_le_bytes())?;
    let mut buffer = vec![0u8; 256 * 1024];
    for (path, size) in files {
        let mut file = std::fs::File::open(root.join(path))?;
        out.write_all(&(path.len() as u16).to_le_bytes())?;
        out.write_all(path.as_bytes())?;
        out.write_all(&size.to_le_bytes())?;
        let mut hash = Sha256::new();
        let mut left = *size;
        while left > 0 {
            let want = buffer
                .len()
                .min(usize::try_from(left).unwrap_or(usize::MAX));
            let read = file.read(&mut buffer[..want])?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("{path} got shorter while it was sent"),
                ));
            }
            hash.update(&buffer[..read]);
            out.write_all(&buffer[..read])?;
            left -= read as u64;
        }
        out.write_all(&hash.finalize())?;
    }
    out.write_all(END)?;
    out.flush()
}

/// The header: how many files and bytes follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub files: u64,
    pub bytes: u64,
}

fn read_array<const N: usize>(input: &mut impl Read) -> io::Result<[u8; N]> {
    let mut bytes = [0u8; N];
    input.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn bad(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// A path from the stream as a path under `root`: relative, no `..`, no drive or root.
fn safe_path(root: &Path, path: &str) -> io::Result<PathBuf> {
    if path.is_empty() || path.contains('\\') || path.contains(':') || path.starts_with('/') {
        return Err(bad(format!("refused path {path:?}")));
    }
    let relative = Path::new(path);
    if !relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(bad(format!("refused path {path:?}")));
    }
    Ok(root.join(relative))
}

/// Reads the stream's header.
pub fn read_header(input: &mut impl Read) -> io::Result<Header> {
    if &read_array::<8>(input)? != MAGIC {
        return Err(bad("not a Wisp file stream"));
    }
    let version = u32::from_le_bytes(read_array(input)?);
    if version != VERSION {
        return Err(bad(format!(
            "file stream version {version} is not supported"
        )));
    }
    Ok(Header {
        files: u64::from_le_bytes(read_array(input)?),
        bytes: u64::from_le_bytes(read_array(input)?),
    })
}

/// Reads the files after [`read_header`] into `root`, calling `progress` with the bytes written
/// so far. Stops with an error at the first bad path, short file or wrong hash.
pub fn read_files(
    input: &mut impl Read,
    header: Header,
    root: &Path,
    progress: &mut dyn FnMut(u64),
) -> io::Result<()> {
    let mut done = 0u64;
    let mut buffer = vec![0u8; 256 * 1024];
    for _ in 0..header.files {
        let length = usize::from(u16::from_le_bytes(read_array(input)?));
        if length > MAX_PATH {
            return Err(bad("path too long"));
        }
        let mut name = vec![0u8; length];
        input.read_exact(&mut name)?;
        let name = String::from_utf8(name).map_err(|_| bad("path is not UTF-8"))?;
        let target = safe_path(root, &name)?;
        let size = u64::from_le_bytes(read_array(input)?);
        if done.saturating_add(size) > header.bytes {
            return Err(bad("more bytes than the header said"));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = io::BufWriter::new(std::fs::File::create(&target)?);
        let mut hash = Sha256::new();
        let mut left = size;
        while left > 0 {
            let want = buffer
                .len()
                .min(usize::try_from(left).unwrap_or(usize::MAX));
            input.read_exact(&mut buffer[..want])?;
            hash.update(&buffer[..want]);
            file.write_all(&buffer[..want])?;
            left -= want as u64;
            done += want as u64;
            progress(done);
        }
        file.flush()?;
        if read_array::<32>(input)? != hash.finalize()[..] {
            return Err(bad(format!("{name} arrived damaged")));
        }
    }
    if &read_array::<8>(input)? != END {
        return Err(bad("the file stream did not end where it should"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wisp-phone-link-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn stream_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(files.len() as u64).to_le_bytes());
        let total: u64 = files.iter().map(|(_, d)| d.len() as u64).sum();
        out.extend_from_slice(&total.to_le_bytes());
        for (path, data) in files {
            out.extend_from_slice(&(path.len() as u16).to_le_bytes());
            out.extend_from_slice(path.as_bytes());
            out.extend_from_slice(&(data.len() as u64).to_le_bytes());
            out.extend_from_slice(data);
            out.extend_from_slice(&Sha256::digest(data));
        }
        out.extend_from_slice(END);
        out
    }

    #[test]
    fn a_folder_arrives_byte_for_byte() {
        let from = scratch("from");
        std::fs::create_dir_all(from.join("assets/minecraft")).unwrap();
        std::fs::write(from.join("wisp-resources.json"), b"{\"v\":1}").unwrap();
        let big: Vec<u8> = (0..700_000u32).map(|i| (i * 7) as u8).collect();
        std::fs::write(from.join("assets/minecraft/big.bin"), &big).unwrap();
        std::fs::write(from.join("assets/empty"), b"").unwrap();
        let files = list(&from).unwrap();
        assert_eq!(
            files.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
            [
                "assets/empty",
                "assets/minecraft/big.bin",
                "wisp-resources.json"
            ]
        );
        let mut stream = Vec::new();
        write(&from, &files, &mut stream).unwrap();

        let to = scratch("to");
        let mut input = stream.as_slice();
        let header = read_header(&mut input).unwrap();
        assert_eq!(header.files, 3);
        assert_eq!(header.bytes, 700_000 + 7);
        let mut last = 0;
        read_files(&mut input, header, &to, &mut |done| last = done).unwrap();
        assert_eq!(last, header.bytes);
        assert_eq!(
            std::fs::read(to.join("assets/minecraft/big.bin")).unwrap(),
            big
        );
        assert_eq!(std::fs::read(to.join("assets/empty")).unwrap(), b"");
        assert_eq!(
            std::fs::read(to.join("wisp-resources.json")).unwrap(),
            b"{\"v\":1}"
        );
    }

    #[test]
    fn paths_outside_the_folder_are_refused() {
        for path in [
            "../evil",
            "a/../../evil",
            "/etc/evil",
            "C:/evil",
            "a\\b",
            "",
        ] {
            let stream = stream_of(&[(path, b"x")]);
            let to = scratch("evil");
            let mut input = stream.as_slice();
            let header = read_header(&mut input).unwrap();
            let error = read_files(&mut input, header, &to, &mut |_| {}).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{path}");
        }
    }

    #[test]
    fn a_damaged_file_is_refused() {
        let mut stream = stream_of(&[("a.txt", b"hello")]);
        // Flip a data byte: the hash no longer matches.
        let at = stream.windows(5).position(|w| w == b"hello").unwrap();
        stream[at] = b'j';
        let to = scratch("damaged");
        let mut input = stream.as_slice();
        let header = read_header(&mut input).unwrap();
        let error = read_files(&mut input, header, &to, &mut |_| {}).unwrap_err();
        assert!(error.to_string().contains("damaged"), "{error}");
    }

    #[test]
    fn a_cut_stream_is_an_error() {
        let stream = stream_of(&[("a.txt", b"hello world")]);
        let cut = &stream[..stream.len() - 20];
        let to = scratch("cut");
        let mut input = cut;
        let header = read_header(&mut input).unwrap();
        assert!(read_files(&mut input, header, &to, &mut |_| {}).is_err());
    }
}
