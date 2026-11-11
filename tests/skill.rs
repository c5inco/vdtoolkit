//! Keeps the agent skill in `skills/vdt/SKILL.md` and the pages of `vdt guide`
//! in step with the command line: every command and flag they name must exist
//! in `vdt --help`, every guide topic they name must exist, and every
//! diagnostic code must have a `vdt explain` entry.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

const SKILL: &str = include_str!("../skills/vdt/SKILL.md");

/// Every page `vdt guide` and `vdt explain` print, by file name.
const GUIDE: [(&str, &str); 8] = [
    ("workflow.md", include_str!("../src/guide/workflow.md")),
    ("adaptive.md", include_str!("../src/guide/adaptive.md")),
    (
        "notification.md",
        include_str!("../src/guide/notification.md"),
    ),
    ("drawables.md", include_str!("../src/guide/drawables.md")),
    ("compose.md", include_str!("../src/guide/compose.md")),
    (
        "diagnostics.md",
        include_str!("../src/guide/diagnostics.md"),
    ),
    ("ci.md", include_str!("../src/guide/ci.md")),
    ("codes.md", include_str!("../src/guide/codes.md")),
];

const ANALYSIS: &str = include_str!("../src/analysis.rs");

fn help(command: Option<&str>) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(command)
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success(), "vdt {command:?} --help failed");
    String::from_utf8(output.stdout).unwrap()
}

/// Flags in a help page or a command line, such as `--fit` and `-o`.
fn flags(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|token| is_flag(token))
        .map(str::to_owned)
        .collect()
}

fn is_flag(token: &str) -> bool {
    let name = token.trim_start_matches('-');
    let dashes = token.len() - name.len();
    name.starts_with(|c: char| c.is_ascii_alphabetic())
        && (dashes == 2 || (dashes == 1 && name.len() == 1))
}

/// Flags each subcommand accepts, keyed by name, with the top-level flags
/// under the empty name.
fn known_flags() -> BTreeMap<String, BTreeSet<String>> {
    let top = help(None);
    let mut known = BTreeMap::new();
    let commands = top
        .lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help");
    for name in commands {
        known.insert(name.to_owned(), flags(&help(Some(name))));
    }
    known.insert(String::new(), flags(&top));
    known
}

/// Every `vdt ...` command line in fenced code blocks and inline code.
fn command_lines(markdown: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut fenced = false;
    let mut pending = String::new();
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            let line = line.split(" #").next().unwrap().trim();
            pending.push_str(line.trim_end_matches('\\'));
            pending.push(' ');
            if !line.ends_with('\\') {
                lines.push(std::mem::take(&mut pending));
            }
        } else {
            lines.extend(line.split('`').skip(1).step_by(2).map(str::to_owned));
        }
    }
    lines
        .into_iter()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| line.starts_with("vdt ") || is_flag(first_word(line)))
        .collect()
}

fn first_word(line: &str) -> &str {
    line.split_whitespace().next().unwrap_or("")
}

/// Commands and flags in `markdown` that `vdt` does not have.
fn unknown(markdown: &str, known: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    let every_flag: BTreeSet<_> = known.values().flatten().cloned().collect();
    let mut problems = Vec::new();
    for line in command_lines(markdown) {
        let words: Vec<_> = line.split_whitespace().collect();
        if words[0] != "vdt" {
            // A flag named on its own in prose, such as `--fit 66`.
            if !every_flag.contains(words[0]) {
                problems.push(format!("unknown flag {} in `{line}`", words[0]));
            }
            continue;
        }
        let (command, rest) = match words.get(1) {
            Some(word) if known.contains_key(*word) => (*word, &words[2..]),
            Some(word) if word.starts_with('-') || word.starts_with('<') => ("", &words[1..]),
            // `vdt icon.svg` is shorthand for `vdt convert icon.svg`.
            _ => ("convert", &words[1..]),
        };
        let accepted = &known[command];
        for flag in flags(&rest.join(" ")) {
            if !accepted.contains(&flag) {
                problems.push(format!("unknown flag {flag} in `{line}`"));
            }
        }
    }
    problems
}

#[test]
fn skill_names_only_commands_and_flags_vdt_has() {
    let problems = unknown(SKILL, &known_flags());
    assert!(
        problems.is_empty(),
        "skills/vdt/SKILL.md is out of date with `vdt --help`:\n{}",
        problems.join("\n")
    );
}

#[test]
fn guide_names_only_commands_and_flags_vdt_has() {
    let known = known_flags();
    let problems: Vec<_> = GUIDE
        .iter()
        .flat_map(|(name, page)| {
            unknown(page, &known)
                .into_iter()
                .map(move |problem| format!("src/guide/{name}: {problem}"))
        })
        .collect();
    assert!(
        problems.is_empty(),
        "the guide is out of date with `vdt --help`:\n{}",
        problems.join("\n")
    );
}

#[test]
fn guide_topics_named_anywhere_exist() {
    let pages = GUIDE.iter().map(|(_, page)| *page).chain([SKILL]);
    let mut topics = BTreeSet::new();
    for page in pages {
        for line in command_lines(page) {
            if let Some(topic) = line.strip_prefix("vdt guide ") {
                topics.insert(topic.split_whitespace().next().unwrap().to_owned());
            }
        }
    }
    assert!(topics.len() >= 5, "found only {topics:?}");
    for topic in topics {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(["guide", &topic])
            .output()
            .unwrap();
        assert!(output.status.success(), "`vdt guide {topic}` failed");
        assert!(!output.stdout.is_empty());
    }
}

/// `VDTnnn` codes in `text` that directly follow `prefix`.
fn codes(text: &str, prefix: &str) -> BTreeSet<String> {
    text.split(prefix)
        .skip(1)
        .filter_map(|rest| rest.get(..6))
        .filter(|code| code.starts_with("VDT") && code[3..].bytes().all(|b| b.is_ascii_digit()))
        .map(str::to_owned)
        .collect()
}

/// The page named `name`, which the tests index by file name so adding a page
/// cannot silently point a test at the wrong one.
fn page(name: &str) -> &'static str {
    GUIDE
        .iter()
        .find(|(file, _)| *file == name)
        .map(|(_, page)| *page)
        .expect("the guide has that page")
}

#[test]
fn every_diagnostic_code_has_an_explanation() {
    let defined = codes(ANALYSIS, "#[serde(rename = \"");
    let explained = codes(page("codes.md"), "\n## ");
    assert_eq!(defined.len(), 27, "expected VDT001 through VDT027");
    assert_eq!(
        defined, explained,
        "src/guide/codes.md and DiagnosticCode differ"
    );
    for code in &defined {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(["explain", code])
            .output()
            .unwrap();
        assert!(output.status.success(), "`vdt explain {code}` failed");
    }
}

#[test]
fn skill_check_reports_invented_flags() {
    let known = known_flags();
    let markdown = "Run `vdt inspect a.svg --colour red`, then `--bogus`.\n\n\
                    ```sh\nvdt adaptive --foreground a.svg \\\n    --size 24 -o res\n\
                    vdt a.svg --fit 20 # shorthand for convert\n```\n";
    assert_eq!(
        unknown(markdown, &known),
        [
            "unknown flag --colour in `vdt inspect a.svg --colour red`",
            "unknown flag --bogus in `--bogus`",
            "unknown flag --size in `vdt adaptive --foreground a.svg --size 24 -o res`",
            "unknown flag --fit in `vdt a.svg --fit 20`",
        ]
    );
    assert!(
        unknown(
            "`vdt --version` and `vdt notification a.svg --fit 20`",
            &known
        )
        .is_empty()
    );
}

#[test]
fn skill_has_frontmatter_agents_require() {
    let frontmatter = SKILL
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(frontmatter, _)| frontmatter)
        .expect("SKILL.md starts with --- frontmatter");
    assert!(frontmatter.lines().any(|line| line == "name: vdt"));
    assert!(
        frontmatter
            .lines()
            .any(|line| line.starts_with("description: ") && line.len() > 100)
    );
}
