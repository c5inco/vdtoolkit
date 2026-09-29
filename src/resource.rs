//! Android resource names, shared by the command-line interface and the
//! WebAssembly bindings so a file and a Figma layer with the same name are
//! written under the same resource name.

use unicode_normalization::UnicodeNormalization;

/// Java keywords and literals, which aapt2 rejects as resource names because
/// they cannot be fields of the generated `R` class.
const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "true",
    "try",
    "void",
    "volatile",
    "while",
];

/// Whether `name` is a valid Android resource name: `[a-z_][a-z0-9_]*`, and
/// not a Java keyword.
pub fn is_resource_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && name != "_"
        && !JAVA_KEYWORDS.contains(&name)
}

/// Turn a file name into a valid Android resource name. A valid name is kept;
/// otherwise words become lowercase and are joined by underscores, as
/// `Arrow-Left` becomes `arrow_left` and `HTTPServer` becomes `http_server`,
/// and a name that would start with a digit or be a Java keyword gains an
/// `ic_` prefix. Accents are dropped, so `Café` becomes `cafe`, and symbols
/// that carry meaning become words, so `C++` becomes `c_plus_plus` rather than
/// colliding with `C`. `None` when the name has no letters or digits to keep.
pub fn resource_name(stem: &str) -> Option<String> {
    if is_resource_name(stem) {
        return Some(stem.to_owned());
    }
    let mut chars = Vec::with_capacity(stem.len());
    for c in stem.nfkd().filter(|c| !('\u{300}'..='\u{36f}').contains(c)) {
        let word = match c {
            '+' => "plus",
            '#' => "sharp",
            '&' => "and",
            '@' => "at",
            '%' => "percent",
            _ => {
                chars.push(c);
                continue;
            }
        };
        chars.push('_');
        chars.extend(word.chars());
        chars.push('_');
    }
    let mut name = String::with_capacity(stem.len());
    for (index, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let previous = index.checked_sub(1).map(|previous| chars[previous]);
            let next = chars.get(index + 1);
            // A word starts after a lowercase letter or digit, and at the last
            // capital of an acronym followed by a lowercase word.
            let word_start = previous
                .is_some_and(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit())
                || (previous.is_some_and(|previous| previous.is_ascii_uppercase())
                    && next.is_some_and(char::is_ascii_lowercase));
            if word_start {
                name.push('_');
            }
            name.push(c.to_ascii_lowercase());
        } else if c.is_ascii_lowercase() || c.is_ascii_digit() {
            name.push(c);
        } else if !name.ends_with('_') {
            name.push('_');
        }
    }
    let name = name.trim_matches('_');
    if name.is_empty() {
        return None;
    }
    Some(
        if name.starts_with(|c: char| c.is_ascii_digit()) || JAVA_KEYWORDS.contains(&name) {
            format!("ic_{name}")
        } else {
            name.to_owned()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_valid_resource_names() {
        for (requested, expected) in [
            ("arrow_left", "arrow_left"),
            ("_private", "_private"),
            ("Icons/Arrow Left", "icons_arrow_left"),
            ("ChevronDown", "chevron_down"),
            ("HTTPServer", "http_server"),
            ("iOSIcon", "i_os_icon"),
            ("24px Café", "ic_24px_cafe"),
            ("C++", "c_plus_plus"),
            ("C#", "c_sharp"),
            ("R&D @ 100%", "r_and_d_at_100_percent"),
            ("class", "ic_class"),
            ("New", "ic_new"),
        ] {
            assert_eq!(
                resource_name(requested).as_deref(),
                Some(expected),
                "{requested}"
            );
            assert!(is_resource_name(expected), "{expected}");
        }
    }

    #[test]
    fn names_without_letters_or_digits_have_no_resource_name() {
        for requested in ["", "_", "🙂", "--"] {
            assert_eq!(resource_name(requested), None, "{requested:?}");
        }
    }
}
