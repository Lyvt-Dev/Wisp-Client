//! Visible copy remains behind a translation seam for the next launcher iteration.
pub fn tr(text: &str) -> String {
    text.to_owned()
}

pub fn trf(text: &str, arguments: &[(&str, &str)]) -> String {
    arguments
        .iter()
        .fold(tr(text), |value, (key, replacement)| {
            value.replace(&format!("{{{key}}}"), replacement)
        })
}
