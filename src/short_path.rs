//! The shortest spelling of path data that Android reads back as the same
//! points, used when writing optimized drawables.
//!
//! Each command is written in whichever of its absolute, relative, and
//! horizontal or vertical forms is shortest. A command letter is dropped when
//! bare numbers continue the same command, the separator is dropped before a
//! minus sign, and leading zeros are dropped. Every Android parser accepts
//! these, back to API 21.
//!
//! Two shortcuts that newer parsers accept are deliberately not used. Android
//! 5.0 cannot read `1.5.5` as two numbers, and framework parsers draw extra
//! coordinates after `M` as further moves rather than line-tos, so a line after
//! a move always gets its own letter.
//!
//! A relative or horizontal or vertical spelling is used only when Android,
//! adding it up in `f32` as its parser does, reaches exactly the number the
//! absolute spelling gives. Coordinates one float step apart describe the same
//! shape, but rendering on Android showed they can change how Skia anti-aliases
//! its edges, so exact numbers keep a drawable rendering identically. The
//! command after a close is always absolute, because Android 5.0 does not
//! return the current point to the subpath's start.

use crate::vector::PathCommand;
use crate::xml::number;

pub(crate) fn write(commands: &[PathCommand]) -> String {
    let mut writer = Writer::default();
    for command in commands {
        writer.command(command);
    }
    writer.text
}

#[derive(Default)]
struct Writer {
    text: String,
    /// The current point as Android's parser computes it from `text`.
    x: f32,
    y: f32,
    /// Where the current subpath started, which `Z` returns to.
    start_x: f32,
    start_y: f32,
    /// The command that bare numbers written next would continue. After a
    /// move, a close, or at the start, a letter is required.
    implicit: Option<char>,
    /// The last number written, when the text ends with one.
    last_number: Option<String>,
    /// Whether the last command was a close, after which only absolute forms
    /// are safe.
    after_close: bool,
}

/// One way to write a command, and the current point Android reaches after it.
struct Form {
    letter: char,
    values: Vec<String>,
    x: f32,
    y: f32,
}

impl Writer {
    fn command(&mut self, command: &PathCommand) {
        let forms = match *command {
            PathCommand::Close => {
                self.text.push('Z');
                self.x = self.start_x;
                self.y = self.start_y;
                self.implicit = None;
                self.last_number = None;
                self.after_close = true;
                return;
            }
            PathCommand::Move(x, y) => self.endpoint_forms('M', &[], x, y),
            PathCommand::Line(x, y) => {
                let mut forms = self.endpoint_forms('L', &[], x, y);
                let (x_text, y_text) = (forms[0].values[0].clone(), forms[0].values[1].clone());
                forms.extend(self.axis_forms(&x_text, &y_text));
                forms
            }
            PathCommand::Quad(a, b, x, y) => self.endpoint_forms('Q', &[(a, b)], x, y),
            PathCommand::Cubic(a, b, c, d, x, y) => {
                self.endpoint_forms('C', &[(a, b), (c, d)], x, y)
            }
        };
        // The first shortest form wins, so ties keep the absolute spelling. Only
        // the winner's text is built.
        let form = forms
            .into_iter()
            .min_by_key(|form| self.spelled_len(form))
            .expect("every drawing command has at least one form");
        let spelling = self.spell(&form);
        debug_assert_eq!(spelling.len(), self.spelled_len(&form));

        self.text.push_str(&spelling);
        self.x = form.x;
        self.y = form.y;
        if form.letter.eq_ignore_ascii_case(&'M') {
            self.start_x = form.x;
            self.start_y = form.y;
        }
        // Framework parsers read extra coordinates after a move as more moves.
        self.implicit = (!form.letter.eq_ignore_ascii_case(&'M')).then_some(form.letter);
        self.last_number = form.values.last().map(|value| without_leading_zero(value));
        self.after_close = false;
    }

    /// The absolute and relative forms of a command given by its control
    /// points and end point.
    fn endpoint_forms(&self, letter: char, controls: &[(f32, f32)], x: f32, y: f32) -> Vec<Form> {
        let absolute: Vec<String> = controls
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .chain([x, y])
            .map(number)
            .collect();
        let relative: Option<Vec<String>> = if self.after_close {
            None
        } else {
            let origins = [self.x, self.y].into_iter().cycle();
            absolute
                .iter()
                .zip(origins)
                .map(|(text, origin)| relative_text(origin, text))
                .collect()
        };
        let mut forms = vec![self.form(letter, absolute, (0.0, 0.0))];
        if let Some(relative) = relative {
            forms.push(self.form(letter.to_ascii_lowercase(), relative, (self.x, self.y)));
        }
        forms
    }

    fn form(&self, letter: char, values: Vec<String>, origin: (f32, f32)) -> Form {
        let end = &values[values.len() - 2..];
        Form {
            x: origin.0 + read(&end[0]),
            y: origin.1 + read(&end[1]),
            letter,
            values,
        }
    }

    /// Horizontal and vertical forms of a line whose end shares the current
    /// point's row or column, as far as the written numbers can tell.
    fn axis_forms(&self, x_text: &str, y_text: &str) -> Vec<Form> {
        let mut forms = Vec::new();
        if self.after_close {
            return forms;
        }
        let (x, y) = (read(x_text), read(y_text));
        if self.y == y {
            forms.push(Form {
                letter: 'H',
                x,
                y: self.y,
                values: vec![x_text.to_owned()],
            });
            if let Some(relative) = relative_text(self.x, x_text) {
                forms.push(Form {
                    letter: 'h',
                    x: self.x + read(&relative),
                    y: self.y,
                    values: vec![relative],
                });
            }
        }
        if self.x == x {
            forms.push(Form {
                letter: 'V',
                x: self.x,
                y,
                values: vec![y_text.to_owned()],
            });
            if let Some(relative) = relative_text(self.y, y_text) {
                forms.push(Form {
                    letter: 'v',
                    x: self.x,
                    y: self.y + read(&relative),
                    values: vec![relative],
                });
            }
        }
        forms
    }

    /// The length of `spell(form)`, counted without building the text.
    fn spelled_len(&self, form: &Form) -> usize {
        let continues = self.implicit == Some(form.letter) && self.last_number.is_some();
        let mut length = usize::from(!continues);
        for (index, value) in form.values.iter().enumerate() {
            let leading_zero = value.starts_with("0.") || value.starts_with("-0.");
            let separated = (index > 0 || continues) && !value.starts_with('-');
            length += value.len() - usize::from(leading_zero) + usize::from(separated);
        }
        length
    }

    /// The text a form adds after what has been written so far.
    fn spell(&self, form: &Form) -> String {
        let mut text = String::new();
        let mut previous = if self.implicit == Some(form.letter) {
            self.last_number.clone()
        } else {
            text.push(form.letter);
            None
        };
        for value in &form.values {
            let value = without_leading_zero(value);
            if previous
                .as_deref()
                .is_some_and(|previous| needs_separator(previous, &value))
            {
                text.push(' ');
            }
            text.push_str(&value);
            previous = Some(value);
        }
        text
    }
}

/// Every Android parser starts a new number at a minus sign. Android 5.0 does
/// not start one at a second decimal point, so `1.5 .5` keeps its space.
fn needs_separator(_previous: &str, next: &str) -> bool {
    !next.starts_with('-')
}

fn without_leading_zero(value: &str) -> String {
    if let Some(rest) = value.strip_prefix("0.") {
        format!(".{rest}")
    } else if let Some(rest) = value.strip_prefix("-0.") {
        format!("-.{rest}")
    } else {
        value.to_owned()
    }
}

fn read(value: &str) -> f32 {
    value.parse().expect("formatted numbers parse")
}

/// The shortest number that, added to `origin` in `f32` as Android does, gives
/// exactly the number Android reads from the `absolute` spelling, if any does.
/// Subtracting coordinates in `f32` leaves noise, such as `0.299999` for
/// `17.5 - 17.2`, so fewer decimals are tried first. Candidates are checked in
/// `f32` before being formatted, because formatting dominates the cost.
fn relative_text(origin: f32, absolute: &str) -> Option<String> {
    let exact = read(absolute);
    let delta = exact - origin;
    (0..=3)
        .map(|decimals| {
            let scale = 10f32.powi(decimals);
            (delta * scale).round() / scale
        })
        .chain([delta])
        .filter(|&candidate| origin + candidate == exact)
        .map(number)
        .find(|text| origin + read(text) == exact)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::PathCommand::{Close, Cubic, Line, Move, Quad};

    /// Read path data as the strictest Android parser does: numbers split at
    /// spaces, commas, and a minus sign after the first character, but a second
    /// decimal point is an error (Android 5.0); extra coordinate sets repeat a
    /// command, and after a move they are further moves (framework parsers);
    /// relative values add to the current point in `f32`; and `Z` leaves the
    /// current point where the subpath ended (Android 5.0).
    fn parse(text: &str) -> Vec<PathCommand> {
        let mut commands = Vec::new();
        let (mut x, mut y) = (0.0f32, 0.0f32);
        let mut rest = text;
        while let Some(letter) = rest.chars().next() {
            assert!(
                letter.is_ascii_alphabetic(),
                "expected a command at {rest:?} in {text:?}"
            );
            let end = rest[1..]
                .find(|c: char| c.is_ascii_alphabetic())
                .map_or(rest.len(), |at| at + 1);
            let values = numbers(&rest[1..end]);
            rest = &rest[end..];
            let arity = match letter.to_ascii_uppercase() {
                'Z' => 0,
                'H' | 'V' => 1,
                'M' | 'L' => 2,
                'Q' => 4,
                'C' => 6,
                other => panic!("unexpected command {other} in {text:?}"),
            };
            if arity == 0 {
                assert!(values.is_empty(), "Z takes no numbers in {text:?}");
                commands.push(Close);
                continue;
            }
            assert!(
                !values.is_empty() && values.len() % arity == 0,
                "{letter} got {} numbers in {text:?}",
                values.len()
            );
            for set in values.chunks(arity) {
                let (ox, oy) = if letter.is_ascii_lowercase() {
                    (x, y)
                } else {
                    (0.0, 0.0)
                };
                match letter.to_ascii_uppercase() {
                    'M' => {
                        (x, y) = (ox + set[0], oy + set[1]);
                        commands.push(Move(x, y));
                    }
                    'L' => {
                        (x, y) = (ox + set[0], oy + set[1]);
                        commands.push(Line(x, y));
                    }
                    'H' => {
                        x = ox + set[0];
                        commands.push(Line(x, y));
                    }
                    'V' => {
                        y = oy + set[0];
                        commands.push(Line(x, y));
                    }
                    'Q' => {
                        commands.push(Quad(ox + set[0], oy + set[1], ox + set[2], oy + set[3]));
                        (x, y) = (ox + set[2], oy + set[3]);
                    }
                    _ => {
                        commands.push(Cubic(
                            ox + set[0],
                            oy + set[1],
                            ox + set[2],
                            oy + set[3],
                            ox + set[4],
                            oy + set[5],
                        ));
                        (x, y) = (ox + set[4], oy + set[5]);
                    }
                }
            }
        }
        commands
    }

    fn numbers(segment: &str) -> Vec<f32> {
        let mut values = Vec::new();
        let mut token = String::new();
        let mut flush = |token: &mut String| {
            if !token.is_empty() {
                values.push(
                    token
                        .parse::<f32>()
                        .unwrap_or_else(|_| panic!("bad number {token:?}")),
                );
                token.clear();
            }
        };
        for c in segment.chars() {
            match c {
                ' ' | ',' => flush(&mut token),
                '-' if !token.is_empty() && !token.ends_with(['e', 'E']) => {
                    flush(&mut token);
                    token.push(c);
                }
                // Android 5.0 keeps a second decimal point in the same number, which then fails to parse.
                _ => token.push(c),
            }
        }
        flush(&mut token);
        values
    }

    fn coordinates(command: &PathCommand) -> Vec<f32> {
        match *command {
            Move(x, y) | Line(x, y) => vec![x, y],
            Quad(a, b, x, y) => vec![a, b, x, y],
            Cubic(a, b, c, d, x, y) => vec![a, b, c, d, x, y],
            Close => vec![],
        }
    }

    /// Android must read exactly the same numbers from the short spelling as
    /// from the long, absolute one, so the drawable renders identically.
    fn assert_reads_back(commands: &[PathCommand]) -> String {
        let short = write(commands);
        let long = crate::xml::path_data(commands);
        let (from_long, from_short) = (parse(&long), parse(&short));
        assert_eq!(from_long.len(), commands.len(), "{long}");
        assert_eq!(from_short.len(), commands.len(), "{short}");
        for (index, (expected, actual)) in from_long.iter().zip(&from_short).enumerate() {
            assert_eq!(
                std::mem::discriminant(expected),
                std::mem::discriminant(actual),
                "command {index} in {short}"
            );
            assert_eq!(
                coordinates(expected),
                coordinates(actual),
                "command {index}: {long} reads differently from {short}"
            );
        }
        assert!(short.len() <= long.len(), "{short} is longer than {long}");
        short
    }

    #[test]
    fn a_rectangle_uses_axis_lines_and_no_spaces_it_does_not_need() {
        let commands = [Move(0.0, 0.0), Line(20.0, 0.0), Line(20.0, 10.0), Close];
        assert_eq!(assert_reads_back(&commands), "M0 0H20V10Z");
    }

    #[test]
    fn minus_signs_separate_numbers_but_second_decimal_points_do_not() {
        let commands = [Move(0.5, 0.25), Line(-0.5, 0.75)];
        assert_eq!(assert_reads_back(&commands), "M.5 .25l-1 .5");
    }

    #[test]
    fn a_line_after_a_move_keeps_its_letter() {
        let commands = [Move(0.0, 0.0), Line(20.0, 20.0), Line(40.0, 40.0)];
        assert_eq!(assert_reads_back(&commands), "M0 0L20 20 40 40");
    }

    #[test]
    fn repeated_commands_drop_their_letter() {
        let commands = [
            Move(30.0, 60.0),
            Cubic(31.0, 61.0, 32.0, 62.0, 33.0, 63.0),
            Cubic(34.0, 64.0, 35.0, 65.0, 36.0, 66.0),
        ];
        assert_eq!(
            assert_reads_back(&commands),
            "M30 60c1 1 2 2 3 3 1 1 2 2 3 3"
        );
    }

    #[test]
    fn relative_steps_drop_the_noise_of_subtracting_rounded_coordinates() {
        // 17.5 - 17.2 is 0.29999924 in f32, which six decimals spell 0.299999.
        let commands = [Move(10.0, 17.2), Line(10.0, 17.5)];
        assert_eq!(assert_reads_back(&commands), "M10 17.2v.3");
    }

    #[test]
    fn the_command_after_a_close_is_absolute() {
        let commands = [
            Move(10.0, 10.0),
            Line(14.0, 10.0),
            Line(14.0, 14.0),
            Close,
            Move(11.0, 11.0),
            Line(12.0, 12.0),
        ];
        let short = assert_reads_back(&commands);
        assert!(short.contains("ZM11 11"), "{short}");
    }

    #[test]
    fn many_small_relative_steps_do_not_drift() {
        let mut commands = vec![Move(0.001, 0.0)];
        for step in 1..5_000 {
            let value = step as f32 * 0.1 + 0.001;
            commands.push(Line(
                (value * 1000.0).round() / 1000.0,
                ((step % 7) as f32 * 0.333).round(),
            ));
        }
        assert_reads_back(&commands);
    }

    /// Deterministic pseudo-random paths: coordinates rounded as `optimize`
    /// leaves them at several scales, plus unrounded ones, with lines that
    /// share a row or column, curves, closes, and new subpaths.
    #[test]
    fn generated_paths_read_back_as_the_same_points_and_are_never_longer() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f32 / (1u64 << 53) as f32
        };
        for (scale, rounded) in [
            (1.0, true),
            (24.0, true),
            (108.0, true),
            (4096.0, true),
            (500.0, false),
        ] {
            for _ in 0..300 {
                let coordinate = |next: &mut dyn FnMut() -> f32| {
                    let value = (next() * 1.2 - 0.1) * scale;
                    if rounded {
                        (value * 1000.0).round() / 1000.0
                    } else {
                        value
                    }
                };
                let mut commands = vec![Move(coordinate(&mut next), coordinate(&mut next))];
                let (mut x, mut y) = match commands[0] {
                    Move(x, y) => (x, y),
                    _ => unreachable!(),
                };
                for _ in 0..40 {
                    let command = match (next() * 7.0) as u32 {
                        0 => Line(coordinate(&mut next), y),
                        1 => Line(x, coordinate(&mut next)),
                        2 => Line(coordinate(&mut next), coordinate(&mut next)),
                        3 => Quad(
                            coordinate(&mut next),
                            coordinate(&mut next),
                            coordinate(&mut next),
                            coordinate(&mut next),
                        ),
                        4 => Cubic(
                            coordinate(&mut next),
                            coordinate(&mut next),
                            coordinate(&mut next),
                            coordinate(&mut next),
                            coordinate(&mut next),
                            coordinate(&mut next),
                        ),
                        5 => Close,
                        _ => Move(coordinate(&mut next), coordinate(&mut next)),
                    };
                    if let Some(end) = coordinates(&command)
                        .rchunks(2)
                        .next()
                        .filter(|end| end.len() == 2)
                    {
                        (x, y) = (end[0], end[1]);
                    }
                    commands.push(command);
                }
                assert_reads_back(&commands);
            }
        }
    }

    /// Every SVG fixture in the repository, converted and optimized, reads back
    /// as the same points from its shortened path data.
    #[test]
    fn repository_fixtures_read_back_as_the_same_points() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0;
        let mut pending = vec![root.join("tools"), root.join("tests")];
        while let Some(directory) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|extension| extension == "svg") {
                    let Ok(source) = std::fs::read(&path) else {
                        continue;
                    };
                    let Ok(mut asset) = crate::convert(&source) else {
                        continue;
                    };
                    asset.optimize();
                    for_each_path(&asset.drawable.children, &mut |commands| {
                        assert_reads_back(commands);
                    });
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "no SVG fixtures found under tools/ or tests/");
    }

    fn for_each_path(nodes: &[crate::vector::VectorNode], visit: &mut dyn FnMut(&[PathCommand])) {
        for node in nodes {
            match node {
                crate::vector::VectorNode::Group(group) => for_each_path(&group.children, visit),
                crate::vector::VectorNode::ClipPath(data) => visit(&data.0),
                crate::vector::VectorNode::Path(path) => visit(&path.path_data.0),
            }
        }
    }
}
