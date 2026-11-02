//! `vdt guide` and `vdt explain`: usage guidance compiled into the binary, so
//! it always describes the commands and diagnostics of the vdt that prints it.

use clap::ValueEnum;

/// A page of `vdt guide`.
#[derive(Clone, Copy, ValueEnum)]
pub enum Topic {
    /// Picking a command and the inspect, fix, generate, wire-up workflow.
    Workflow,
    /// Adaptive launcher icons: layers, --fit, --legacy, the manifest.
    Adaptive,
    /// Notification icons: flattening, --fit, setSmallIcon.
    Notification,
    /// Plain drawables: where to write them, size, minimum API.
    Drawables,
    /// Reading inspect reports, severities, and exit codes.
    Diagnostics,
    /// Gating pull requests with check, and reproducible output.
    Ci,
}

impl Topic {
    pub fn page(self) -> &'static str {
        match self {
            Self::Workflow => include_str!("guide/workflow.md"),
            Self::Adaptive => include_str!("guide/adaptive.md"),
            Self::Notification => include_str!("guide/notification.md"),
            Self::Drawables => include_str!("guide/drawables.md"),
            Self::Diagnostics => include_str!("guide/diagnostics.md"),
            Self::Ci => include_str!("guide/ci.md"),
        }
    }
}

const CODES: &str = include_str!("guide/codes.md");

/// Each `## VDTnnn: title` section of the code reference, as `(code, section)`.
fn sections() -> impl Iterator<Item = (&'static str, &'static str)> {
    CODES.split("\n## ").skip(1).map(|section| {
        let code = section.split(':').next().unwrap_or_default();
        (code, section.trim_end())
    })
}

/// The explanation of `code`, which may be written `VDT019`, `vdt019`, or
/// `19`.
pub fn explain(code: &str) -> Option<String> {
    let digits = code
        .trim()
        .trim_start_matches(|c: char| c.is_ascii_alphabetic());
    let number: u32 = digits.parse().ok()?;
    let code = format!("VDT{number:03}");
    sections()
        .find(|(name, _)| *name == code)
        .map(|(_, section)| format!("## {section}\n"))
}

/// One line per code: `VDT019  outside the safe zone`.
pub fn index() -> String {
    let mut out = String::from("Diagnostic codes. `vdt explain <code>` explains one.\n\n");
    for (code, section) in sections() {
        let title = section
            .lines()
            .next()
            .and_then(|line| line.split_once(": "))
            .map_or("", |(_, title)| title);
        out.push_str(&format!("{code}  {title}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_codes_however_they_are_written() {
        for code in ["VDT019", "vdt019", "19", " VDT19 "] {
            let text = explain(code).unwrap();
            assert!(
                text.starts_with("## VDT019: outside the safe zone\n"),
                "{code}"
            );
            assert!(!text.contains("## VDT020"));
        }
        assert!(explain("VDT999").is_none());
        assert!(explain("fit").is_none());
    }

    #[test]
    fn index_lists_every_section() {
        let index = index();
        assert!(index.contains("VDT001  unsupported mask\n"));
        assert!(index.contains("VDT026  foreground opaque\n"));
        assert_eq!(
            index.lines().filter(|l| l.starts_with("VDT")).count(),
            sections().count()
        );
    }
}
