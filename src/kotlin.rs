//! The Kotlin writer the Compose targets share: how a call's arguments are
//! laid out, how imports are collected, and how a number or a path data string
//! is spelled.
//!
//! Both targets write the same language and the same formatting rules, so the
//! machinery that decides where a line breaks and which import a call needs
//! lives here, and each target writes only the calls its own Compose type
//! takes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crate::vector::{Color, GradientStop, MaskKind, Paint, PathCommand, TileMode, VectorNode};

/// Longest line this writer produces before a call's arguments go one per
/// line, matching the width `ktfmt`, which formats generated Kotlin in an
/// Android project, wraps at.
pub(crate) const LINE_WIDTH: usize = 100;

/// Most path commands a drawing may carry and still write each of them as a
/// builder call.
///
/// Kotlin inlines the lambdas the Compose builders take — `path { }`,
/// `group { }`, `apply { }` — so every command becomes bytecode of the one
/// method that holds the drawing, and the JVM refuses a method of over 64 KB.
/// Compiled with Kotlin 2.4.10 against Compose ui 1.7.6, a drawing of 2,500
/// cubic commands compiles and one of 3,000 fails with "Method too large". A
/// drawing over this budget is written as path data strings instead, which is
/// one call per path however long the path is.
pub(crate) const INLINE_COMMAND_BUDGET: usize = 1_000;

/// Whether a drawing is written with its path data as strings rather than as
/// builder calls, which is what keeps a drawing of any size inside the JVM's
/// 64 KB method limit: a string is one statement however long the path is.
pub(crate) fn path_data_strings(nodes: &[VectorNode]) -> bool {
    commands(nodes) > INLINE_COMMAND_BUDGET
}

/// Longest string literal to write for path data. The JVM holds a string
/// constant of at most 64 KB, so a longer path is written as several literals
/// concatenated.
pub(crate) const PATH_DATA_STRING_LIMIT: usize = 60_000;

/// One argument of a Kotlin call.
pub(crate) struct Parameter {
    pub(crate) name: Option<&'static str>,
    /// The value. Lines after the first are already indented, because the
    /// writer that produced them placed them at the depth they belong at.
    pub(crate) value: String,
}

impl Parameter {
    pub(crate) fn named(name: &'static str, value: String) -> Self {
        Self {
            name: Some(name),
            value,
        }
    }

    pub(crate) fn plain(value: String) -> Self {
        Self { name: None, value }
    }

    fn first_line(&self) -> &str {
        self.value.lines().next().unwrap_or_default()
    }

    fn rest(&self) -> impl Iterator<Item = &str> {
        self.value.lines().skip(1)
    }

    /// Width of this argument written on one line, or `None` when one of its
    /// own values spans lines.
    fn width(&self) -> Option<usize> {
        (!self.value.contains('\n'))
            .then(|| self.name.map_or(0, |name| name.len() + 3) + self.value.chars().count())
    }
}

/// How a target's path type spells the two commands the Compose types name
/// differently.
#[derive(Clone, Copy, Default)]
pub(crate) enum PathCommands {
    /// `PathBuilder`, which an `ImageVector` path or `PathData` block takes:
    /// `curveTo` and `quadTo`.
    #[default]
    Builder,
    /// `Path`, which `drawPath` and `clipPath` take: `cubicTo` and
    /// `quadraticTo`.
    Path,
}

#[derive(Default)]
pub(crate) struct Body {
    pub(crate) text: String,
    pub(crate) depth: usize,
    pub(crate) imports: BTreeSet<&'static str>,
    /// Write path data as a string instead of as builder calls, for a drawing
    /// too large to compile otherwise.
    pub(crate) path_data_strings: bool,
    /// Local names this writer has handed out, per prefix, for the values it
    /// has to name to reuse or mutate.
    pub(crate) locals: BTreeMap<&'static str, usize>,
    /// Rectangle a canvas layer has to cover, in the units the drawing is in.
    /// `None` for a writer whose output has no layers.
    pub(crate) layer_bounds: Option<String>,
    /// Top-level declarations the drawing needs beside the class, written
    /// after it in a stable order.
    pub(crate) declarations: BTreeSet<&'static str>,
    /// The names this target's path type gives the cubic and quadratic
    /// commands.
    pub(crate) path_commands: PathCommands,
}

impl Body {
    pub(crate) fn import(&mut self, path: &'static str) {
        self.imports.insert(path);
    }

    /// Write `declaration` as a top-level declaration of the file.
    pub(crate) fn declare(&mut self, declaration: &'static str) {
        self.declarations.insert(declaration);
    }

    pub(crate) fn line_at(&mut self, depth: usize, text: &str) {
        for _ in 0..depth {
            self.text.push_str("    ");
        }
        self.text.push_str(text);
        self.text.push('\n');
    }

    pub(crate) fn line(&mut self, text: &str) {
        self.line_at(self.depth, text);
    }

    /// Append a line that already carries its own indentation, as the
    /// continuation lines of a multi-line argument do.
    pub(crate) fn raw_line(&mut self, text: &str) {
        self.text.push_str(text);
        self.text.push('\n');
    }

    /// Render `block` at `depth` into a `String` instead of the body, keeping
    /// the imports it added.
    pub(crate) fn scratch(&mut self, depth: usize, block: impl FnOnce(&mut Self)) -> String {
        let outer = std::mem::take(&mut self.text);
        let outer_depth = std::mem::replace(&mut self.depth, depth);
        block(self);
        let inner = std::mem::take(&mut self.text);
        self.text = outer;
        self.depth = outer_depth;
        inner
    }

    /// Emit `name(...)suffix`, one argument per line when the single-line form
    /// does not fit the writer's line width.
    pub(crate) fn call(&mut self, name: &str, parameters: &[Parameter], suffix: &str) {
        let arguments = self.single_line_at(parameters, self.depth);
        match arguments {
            Some(arguments) => self.line(&format!("{name}({arguments}){suffix}")),
            None => {
                self.line(&format!("{name}("));
                self.parameters(parameters, self.depth + 1);
                self.line(&format!("){suffix}"));
            }
        }
    }

    /// `name(...)` as the value of a named parameter: joined onto one line
    /// when the whole argument fits, otherwise one argument per line at
    /// `depth`. `columns` is how much of the line is already used before the
    /// value, so that `style = Stroke(...)` is measured with `style = ` in it.
    /// Only the lines after the first carry indentation of their own.
    pub(crate) fn expression(
        &mut self,
        name: &str,
        parameters: &[Parameter],
        depth: usize,
        columns: usize,
    ) -> String {
        // The whole argument is `{columns}{name}({arguments}),`.
        let arguments = self
            .argument_width(parameters)
            .filter(|arguments| columns + name.len() + arguments + 3 <= LINE_WIDTH);
        match arguments {
            Some(_) => format!("{name}({})", self.joined(parameters)),
            None => self.block_call(name, parameters, depth),
        }
    }

    /// A call's arguments joined onto one line.
    fn joined(&self, parameters: &[Parameter]) -> String {
        parameters
            .iter()
            .map(|parameter| match parameter.name {
                Some(name) => format!("{name} = {}", parameter.first_line()),
                None => parameter.first_line().to_owned(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// A call whose arguments always fall one per line, used as the value of a
    /// named parameter. Its first line follows `name = `, so only the lines
    /// inside it carry indentation of their own.
    pub(crate) fn block_call(
        &mut self,
        name: &str,
        parameters: &[Parameter],
        depth: usize,
    ) -> String {
        let inner = self.scratch(depth + 1, |body| body.parameters(parameters, depth + 1));
        format!("{name}(\n{inner}{})", "    ".repeat(depth))
    }

    /// The next unused local name with `prefix`, such as `clip1`.
    pub(crate) fn local(&mut self, prefix: &'static str) -> String {
        let count = self.locals.entry(prefix).or_default();
        *count += 1;
        format!("{prefix}{count}")
    }

    /// The arguments of a call joined onto one line, or `None` when they
    /// cannot be: one of their values spans lines, or the line at `depth`
    /// would be longer than the writer wraps at.
    fn single_line_at(&self, parameters: &[Parameter], depth: usize) -> Option<String> {
        let arguments = self.argument_width(parameters)?;
        if depth * 4 + arguments > LINE_WIDTH {
            return None;
        }
        Some(self.joined(parameters))
    }

    /// Width of a call's arguments written on one line, or `None` when one of
    /// their values spans lines.
    fn argument_width(&self, parameters: &[Parameter]) -> Option<usize> {
        let widths = parameters
            .iter()
            .map(Parameter::width)
            .collect::<Option<Vec<_>>>()?;
        Some(widths.iter().sum::<usize>() + 2 * widths.len().saturating_sub(1))
    }

    /// Every argument on its own line at `depth`, a multi-line value keeping
    /// the indentation its own writer gave it. The separating comma belongs
    /// after the argument's last line, which for a value that spans lines is
    /// its closing bracket.
    fn parameters(&mut self, parameters: &[Parameter], depth: usize) {
        for parameter in parameters {
            let first = match parameter.name {
                Some(name) => format!("{name} = {}", parameter.first_line()),
                None => parameter.first_line().to_owned(),
            };
            let mut rest = parameter.rest().peekable();
            if rest.peek().is_none() {
                self.line_at(depth, &format!("{first},"));
                continue;
            }
            self.line_at(depth, &first);
            while let Some(line) = rest.next() {
                if rest.peek().is_some() {
                    self.raw_line(line);
                } else {
                    self.raw_line(&format!("{line},"));
                }
            }
        }
    }

    /// One `Path` command as the call that appends it.
    pub(crate) fn command(&mut self, command: &PathCommand) {
        fn arguments(values: &[f32]) -> Vec<Parameter> {
            values
                .iter()
                .map(|value| Parameter::plain(format!("{}f", number(*value))))
                .collect()
        }
        let (cubic, quad) = match self.path_commands {
            PathCommands::Builder => ("curveTo", "quadTo"),
            PathCommands::Path => ("cubicTo", "quadraticTo"),
        };
        match *command {
            PathCommand::Move(x, y) => self.call("moveTo", &arguments(&[x, y]), ""),
            PathCommand::Line(x, y) => self.call("lineTo", &arguments(&[x, y]), ""),
            PathCommand::Quad(a, b, x, y) => self.call(quad, &arguments(&[a, b, x, y]), ""),
            PathCommand::Cubic(a, b, c, d, x, y) => {
                self.call(cubic, &arguments(&[a, b, c, d, x, y]), "")
            }
            PathCommand::Close => self.line("close()"),
        }
    }

    /// A fill or stroke as a Compose brush, written at `depth`, the indentation
    /// of the argument it is the value of.
    pub(crate) fn brush(&mut self, paint: &Paint, depth: usize) -> String {
        match paint {
            Paint::Solid(color) => {
                self.import("androidx.compose.ui.graphics.SolidColor");
                format!("SolidColor({})", self.color(*color, 1.0))
            }
            Paint::Linear(gradient) => {
                self.import("androidx.compose.ui.geometry.Offset");
                self.import("androidx.compose.ui.graphics.Brush");
                let mut parameters = vec![
                    Parameter::named("colorStops", self.stops(&gradient.stops, depth + 1)),
                    Parameter::named(
                        "start",
                        format!(
                            "Offset({}f, {}f)",
                            number(gradient.start_x),
                            number(gradient.start_y)
                        ),
                    ),
                    Parameter::named(
                        "end",
                        format!(
                            "Offset({}f, {}f)",
                            number(gradient.end_x),
                            number(gradient.end_y)
                        ),
                    ),
                ];
                parameters.extend(self.tile_mode(gradient.tile_mode));
                self.block_call("Brush.linearGradient", &parameters, depth)
            }
            Paint::Radial(gradient) => {
                self.import("androidx.compose.ui.geometry.Offset");
                self.import("androidx.compose.ui.graphics.Brush");
                let mut parameters = vec![
                    Parameter::named("colorStops", self.stops(&gradient.stops, depth + 1)),
                    Parameter::named(
                        "center",
                        format!(
                            "Offset({}f, {}f)",
                            number(gradient.center_x),
                            number(gradient.center_y)
                        ),
                    ),
                    Parameter::named("radius", format!("{}f", number(gradient.radius))),
                ];
                parameters.extend(self.tile_mode(gradient.tile_mode));
                self.block_call("Brush.radialGradient", &parameters, depth)
            }
        }
    }

    fn tile_mode(&mut self, tile_mode: TileMode) -> Option<Parameter> {
        let value = match tile_mode {
            TileMode::Clamp => return None,
            TileMode::Mirror => "TileMode.Mirror",
            // Compose spells the repeated tile mode `Repeated`.
            TileMode::Repeat => "TileMode.Repeated",
        };
        self.import("androidx.compose.ui.graphics.TileMode");
        Some(Parameter::named("tileMode", value.to_owned()))
    }

    /// The `colorStops` argument of a brush, written at `depth`.
    fn stops(&mut self, stops: &[GradientStop], depth: usize) -> String {
        let parameters = stops
            .iter()
            .map(|stop| {
                let color = self.color(stop.color, stop.alpha);
                Parameter::plain(format!("{}f to {color}", number(stop.offset)))
            })
            .collect::<Vec<_>>();
        self.block_call("arrayOf", &parameters, depth)
    }

    pub(crate) fn color(&mut self, color: Color, alpha: f32) -> String {
        self.import("androidx.compose.ui.graphics.Color");
        format!(
            "Color(0x{:02X}{:02X}{:02X}{:02X})",
            crate::vector::alpha_byte(alpha),
            color.0,
            color.1,
            color.2
        )
    }
}

/// The whole `addPathNodes(…)` call whose argument is the path data as the
/// string Compose parses, written as the expression that ends at `depth`.
///
/// A string constant holds at most 64 KB, so a longer path is split at spaces
/// and concatenated: each piece holds whole numbers, so reading the
/// concatenation back gives exactly what one literal would. The pieces are
/// joined by `+` into the one argument, and the argument list ends with a
/// trailing comma, which Kotlin allows there.
pub(crate) fn path_data_call(text: &str, depth: usize) -> String {
    if text.len() <= PATH_DATA_STRING_LIMIT {
        return format!("addPathNodes(\"{text}\")");
    }
    let mut text = text;
    // One argument, written over several lines: the chunks are joined by `+`,
    // and the trailing comma closes an argument list rather than a parenthesized
    // expression, which `("a" + "b",)` would be and Kotlin would reject.
    let mut expression = String::from("addPathNodes(\n");
    while !text.is_empty() {
        let end = if text.len() <= PATH_DATA_STRING_LIMIT {
            text.len()
        } else {
            text[..PATH_DATA_STRING_LIMIT]
                .rfind(' ')
                .map_or(text.len(), |space| space + 1)
        };
        let (chunk, rest) = text.split_at(end);
        let indent = "    ".repeat(depth + 1);
        let _ = write!(expression, "{indent}\"{chunk}\"");
        expression.push_str(if rest.is_empty() { ",\n" } else { " +\n" });
        text = rest;
    }
    let _ = write!(expression, "{})", "    ".repeat(depth));
    expression
}

/// Number of path commands in a node list, which is what a generated method's
/// size is proportional to.
///
/// Saturating, because a luminance mask is written twice and each nested mask
/// doubles it again: a deep chain of them multiplies past anything a count can
/// mean, and a wrapped one would read as a small drawing that is not.
pub(crate) fn commands(nodes: &[VectorNode]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            VectorNode::Path(path) => path.path_data.0.len(),
            VectorNode::ClipPath(clip) => clip
                .shapes
                .iter()
                .map(|shape| shape.data.0.len())
                .fold(0, usize::saturating_add),
            // A group's mask is drawn in the same method as its children, so it
            // counts toward the budget too.
            VectorNode::Group(group) => {
                let mask = group.mask.as_ref().map_or(0, |mask| {
                    // A luminance mask is drawn twice: once through the colour
                    // matrix that reads the luminance, and once to apply the
                    // alpha that matrix replaced. Both passes draw the region
                    // and the whole content, and each nested mask inside the
                    // content doubles it again, which the recursion has already
                    // counted.
                    let passes: usize = if mask.kind == MaskKind::Luminance {
                        2
                    } else {
                        1
                    };
                    passes.saturating_mul(
                        mask.region
                            .iter()
                            .map(|data| data.0.len())
                            .fold(0, usize::saturating_add)
                            .saturating_add(commands(&mask.children)),
                    )
                });
                commands(&group.children).saturating_add(mask)
            }
        })
        .fold(0, usize::saturating_add)
}

/// Format a viewport value as the digits of a Kotlin `Float` literal, using
/// the same formatter as the XML writer, so every target agrees on every
/// number.
pub(crate) fn number(value: f32) -> String {
    crate::xml::number(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::{
        FillRule, LineCap, LineJoin, MaskKind, PathData, VectorGroup, VectorMask, VectorPath,
    };

    #[test]
    fn commands_saturates_instead_of_wrapping() {
        // A luminance mask is written twice and each nested mask doubles it
        // again, so a deep chain of them multiplies past what a `usize` holds.
        // The count saturates rather than wrapping into a small number that
        // would read as a drawing worth writing.
        let path = VectorPath {
            path_data: PathData(vec![PathCommand::Move(0.0, 0.0)]),
            fill: None,
            fill_alpha: 1.0,
            fill_rule: FillRule::NonZero,
            stroke: None,
            stroke_alpha: 1.0,
            stroke_width: 0.0,
            stroke_cap: LineCap::Butt,
            stroke_join: LineJoin::Miter,
            stroke_miter: 4.0,
            dash: None,
        };
        let mut nodes = vec![VectorNode::Path(path)];
        for _ in 0..70 {
            let mut outer = VectorGroup::new(Vec::new());
            outer.mask = Some(VectorMask {
                kind: MaskKind::Luminance,
                children: vec![VectorNode::Group(VectorGroup::new(nodes))],
                region: None,
            });
            nodes = vec![VectorNode::Group(outer)];
        }
        assert_eq!(commands(&nodes), usize::MAX);
    }
}
