//! Kotlin source for a Compose `ImageVector`, written from the same lowered
//! model as the VectorDrawable writer.
//!
//! Compose draws a vector through `VectorPainter`, which reads an
//! `ImageVector` and has no VectorDrawable parser, so nothing here follows
//! Android's attribute, API level, or `aapt` rules: a gradient, an even-odd
//! fill, and a clip path need no particular API level, and a gradient is an
//! ordinary `Brush` rather than an inline `aapt` complex color. What an
//! `ImageVector` can express is, however, as limited as a VectorDrawable in
//! every way that concerns artwork: a group has no alpha, a path has no dash,
//! blend mode, or `PathEffect`, a brush has no focal point or pattern, and
//! gradients are linear or circular radial, so the set of SVGs that convert is
//! the set `convert` accepts.
//!
//! The output has the shape the material-icons generator and the third-party
//! `svg-to-compose` and Valkyrie converters emit: a lazily initialized
//! top-level property over a private backing field, which is what keeps the
//! raster cache of a `VectorPainter` keyed to one `ImageVector` per
//! recomposition. Parameters that equal the `ImageVector` defaults are left
//! out, which is the same rule the XML writer follows for
//! `android:fillAlpha` and the stroke attributes, so a path reads as the
//! handful of values that differ from what Compose already assumes.
//!
//! Two limits of a Kotlin file shape the output. A `get()` of more than 64 KB
//! of bytecode does not compile, and every command written as a `PathBuilder`
//! call lands in it because Kotlin inlines the builders, so a drawing over
//! [`INLINE_COMMAND_BUDGET`] commands is written as path data strings instead,
//! one call per path. A string constant holds at most 64 KB, so path data
//! longer than [`PATH_DATA_STRING_LIMIT`] is written as concatenated
//! literals. Both limits and the layout machinery behind them live in
//! [`crate::kotlin`], which this writer shares with the draw target.

use std::fmt::Write;

use crate::kotlin::{Body, Parameter, number, path_data_call, path_data_strings};
use crate::vector::{
    FillRule, LineCap, LineJoin, Paint, PathData, VectorDrawable, VectorGroup, VectorNode,
    VectorPath,
};

/// The Kotlin name and package of a generated `ImageVector`.
pub struct Kotlin<'a> {
    /// Name of the generated property, of the `ImageVector`, and of its
    /// backing field. Must be a valid Kotlin identifier.
    pub name: &'a str,
    /// `package` declaration to write. Without it the property lands in the
    /// default package, which no other package can import, so a caller that
    /// knows where the file belongs should pass one.
    pub package: Option<&'a str>,
}

/// Serialize a drawable as Kotlin that builds the equivalent `ImageVector`.
pub fn write(drawable: &VectorDrawable, options: &Kotlin<'_>) -> String {
    // Nodes sit inside `get()`, the assignment, and `.apply {`: three levels
    // of indentation.
    let mut body = Body {
        depth: 3,
        path_data_strings: path_data_strings(&drawable.children),
        ..Body::default()
    };
    body.import("androidx.compose.ui.graphics.vector.ImageVector");
    body.import("androidx.compose.ui.unit.dp");
    // A clip path at the top of a drawable scopes everything after it, so the
    // children become the body of a group that carries that clip. The lowering
    // leaves one there for a legacy launcher icon's circular mask.
    body.scoped(&drawable.children);

    let name = options.name;
    let mut kotlin = String::new();
    if let Some(package) = options.package {
        let _ = writeln!(kotlin, "package {package}");
        kotlin.push('\n');
    }
    for import in &body.imports {
        let _ = writeln!(kotlin, "import {import}");
    }
    let _ = writeln!(kotlin);
    let _ = writeln!(kotlin, "public val {name}: ImageVector");
    let _ = writeln!(kotlin, "    get() {{");
    let _ = writeln!(kotlin, "        if (_{name} != null) {{");
    let _ = writeln!(kotlin, "            return _{name}!!");
    let _ = writeln!(kotlin, "        }}");
    let _ = writeln!(kotlin, "        _{name} = ImageVector.Builder(");
    let _ = writeln!(kotlin, "            name = \"{name}\",");
    let _ = writeln!(
        kotlin,
        "            defaultWidth = {}.dp,",
        number(drawable.width_dp)
    );
    let _ = writeln!(
        kotlin,
        "            defaultHeight = {}.dp,",
        number(drawable.height_dp)
    );
    let _ = writeln!(
        kotlin,
        "            viewportWidth = {}f,",
        number(drawable.viewport_width)
    );
    let _ = writeln!(
        kotlin,
        "            viewportHeight = {}f,",
        number(drawable.viewport_height)
    );
    let _ = writeln!(kotlin, "        ).apply {{");
    kotlin.push_str(&body.text);
    let _ = writeln!(kotlin, "        }}.build()");
    let _ = writeln!(kotlin, "        return _{name}!!");
    let _ = writeln!(kotlin, "    }}");
    let _ = writeln!(kotlin);
    let _ = writeln!(kotlin, "private var _{name}: ImageVector? = null");
    kotlin
}

impl Body {
    fn nodes<'a>(&mut self, nodes: impl IntoIterator<Item = &'a VectorNode>) {
        for node in nodes {
            match node {
                VectorNode::Path(path) => self.path(path),
                // A clip path is emitted as the `clipPathData` of the group it
                // scopes, never as a node of its own.
                VectorNode::ClipPath(_) => {}
                VectorNode::Group(group) => self.group(group),
            }
        }
    }

    /// Whether a node list paints anything. A group whose only child is a clip
    /// path scopes nothing.
    fn paints<'a>(nodes: impl IntoIterator<Item = &'a VectorNode>) -> bool {
        nodes.into_iter().any(|node| match node {
            VectorNode::Path(_) => true,
            VectorNode::ClipPath(_) => false,
            VectorNode::Group(group) => Self::paints(&group.children),
        })
    }

    fn path(&mut self, path: &VectorPath) {
        let mut parameters = self.path_parameters(path);
        if self.path_data_strings {
            // One call per path however many commands the path carries, which
            // is what keeps a large drawing inside the JVM's method limit.
            // `addPath` is a member of the builder, so only the parser behind
            // `addPathNodes` needs an import.
            self.import("androidx.compose.ui.graphics.vector.addPathNodes");
            parameters.insert(
                0,
                Parameter::named("pathData", self.path_data(&path.path_data, self.depth + 1)),
            );
            self.call("addPath", &parameters, "");
            return;
        }
        self.import("androidx.compose.ui.graphics.vector.path");
        self.call("path", &parameters, " {");
        self.depth += 1;
        for command in &path.path_data.0 {
            self.command(command);
        }
        self.depth -= 1;
        self.line("}");
    }

    fn path_parameters(&mut self, path: &VectorPath) -> Vec<Parameter> {
        // The parameter depth a call's arguments land at, which is what a
        // multi-line value inside one of them indents itself by.
        let arguments = self.depth + 1;
        let mut parameters = Vec::new();
        if let Some(paint) = &path.fill {
            parameters.push(Parameter::named("fill", self.brush(paint, arguments)));
        }
        if path.fill_alpha != 1.0 {
            parameters.push(Parameter::named(
                "fillAlpha",
                format!("{}f", number(path.fill_alpha)),
            ));
        }
        if let Some(paint) = &path.stroke {
            parameters.extend(self.stroke_parameters(path, paint, arguments));
        }
        if path.fill_rule == FillRule::EvenOdd {
            self.import("androidx.compose.ui.graphics.PathFillType");
            parameters.push(Parameter::named(
                "pathFillType",
                "PathFillType.EvenOdd".to_owned(),
            ));
        }
        parameters
    }

    /// The arguments of a path's stroke. A stroke of width zero is left out
    /// rather than written as one: SVG and VectorDrawable draw nothing for it,
    /// while Compose draws a hairline, one pixel of the rasterized vector.
    fn stroke_parameters(
        &mut self,
        path: &VectorPath,
        paint: &Paint,
        arguments: usize,
    ) -> Vec<Parameter> {
        let mut parameters = Vec::new();
        if path.stroke_width == 0.0 {
            return parameters;
        }
        parameters.push(Parameter::named("stroke", self.brush(paint, arguments)));
        if path.stroke_alpha != 1.0 {
            parameters.push(Parameter::named(
                "strokeAlpha",
                format!("{}f", number(path.stroke_alpha)),
            ));
        }
        parameters.push(Parameter::named(
            "strokeLineWidth",
            format!("{}f", number(path.stroke_width)),
        ));
        match path.stroke_cap {
            LineCap::Butt => {}
            cap => {
                self.import("androidx.compose.ui.graphics.StrokeCap");
                let value = match cap {
                    LineCap::Round => "StrokeCap.Round",
                    _ => "StrokeCap.Square",
                };
                parameters.push(Parameter::named("strokeLineCap", value.to_owned()));
            }
        }
        match path.stroke_join {
            LineJoin::Miter => {}
            join => {
                self.import("androidx.compose.ui.graphics.StrokeJoin");
                let value = match join {
                    LineJoin::Round => "StrokeJoin.Round",
                    _ => "StrokeJoin.Bevel",
                };
                parameters.push(Parameter::named("strokeLineJoin", value.to_owned()));
            }
        }
        if path.stroke_miter != 4.0 {
            parameters.push(Parameter::named(
                "strokeLineMiter",
                format!("{}f", number(path.stroke_miter)),
            ));
        }
        parameters
    }

    fn group(&mut self, group: &VectorGroup) {
        if !Self::paints(&group.children) {
            return;
        }
        let (clips, children) = split_scope(&group.children);
        if group.is_identity() && clips.is_empty() {
            self.nodes(children);
            return;
        }
        let mut parameters = Vec::new();
        for (name, value, default) in [
            ("rotate", group.rotation, 0.0),
            ("pivotX", group.pivot_x, 0.0),
            ("pivotY", group.pivot_y, 0.0),
            ("scaleX", group.scale_x, 1.0),
            ("scaleY", group.scale_y, 1.0),
            ("translationX", group.translate_x, 0.0),
            ("translationY", group.translate_y, 0.0),
        ] {
            if value != default {
                parameters.push(Parameter::named(name, format!("{}f", number(value))));
            }
        }
        self.scoped_parameters(parameters, &clips, children);
    }

    /// Emit `children` inside the group `parameters` describe, with a leading
    /// clip path scoping them, and one nested group for each further clip.
    ///
    /// Compose clips a group's children with the whole `clipPathData` list at
    /// once, which unions the paths in it, so a second clip has to be its own
    /// nested group to intersect the first, as the nested VectorDrawable
    /// `<group>` elements of the same lowering do.
    fn scoped(&mut self, children: &[VectorNode]) {
        let (clips, children) = split_scope(children);
        self.scoped_parameters(Vec::new(), &clips, children);
    }

    fn scoped_parameters(
        &mut self,
        mut parameters: Vec<Parameter>,
        clips: &[&PathData],
        children: Vec<&VectorNode>,
    ) {
        if clips.is_empty() && parameters.is_empty() {
            self.nodes(children);
            return;
        }
        self.import("androidx.compose.ui.graphics.vector.group");
        let arguments = self.depth + 1;
        if let Some(first) = clips.first() {
            parameters.push(Parameter::named(
                "clipPathData",
                self.clip_path_data(first, arguments),
            ));
        }
        self.call("group", &parameters, " {");
        self.depth += 1;
        let nested = clips.len().saturating_sub(1);
        for rest in clips.iter().skip(1) {
            let data = self.clip_path_data(rest, self.depth + 1);
            self.call("group", &[Parameter::named("clipPathData", data)], " {");
            self.depth += 1;
        }
        self.nodes(children);
        for _ in 0..=nested {
            self.depth -= 1;
            self.line("}");
        }
    }

    /// Path data as the argument `addPathNodes` parses, written at `depth`.
    fn path_data(&mut self, data: &PathData, depth: usize) -> String {
        self.import("androidx.compose.ui.graphics.vector.addPathNodes");
        let text = crate::short_path::write(&data.0);
        path_data_call(&text, depth)
    }

    /// A `clipPathData` argument: the clip's geometry as Compose path nodes.
    /// Every clip path wraps, because a stroke or curve that fits one line
    /// still reads as a group of commands.
    fn clip_path_data(&mut self, clip: &PathData, depth: usize) -> String {
        if self.path_data_strings {
            return self.path_data(clip, depth);
        }
        self.import("androidx.compose.ui.graphics.vector.PathData");
        let body = self.scratch(depth + 1, |body| {
            for command in &clip.0 {
                body.command(command);
            }
        });
        format!("PathData {{\n{body}{}}}", "    ".repeat(depth))
    }
}

/// Split a node list into the clip paths that scope it and what they scope.
///
/// The lowering writes a group's clip paths before anything they scope, and a
/// clip scopes the siblings that follow it, where Compose clips every child of
/// the group that carries it. A clip path that is not in that prefix would
/// therefore scope more through Compose than it does as a drawable.
fn split_scope(nodes: &[VectorNode]) -> (Vec<&PathData>, Vec<&VectorNode>) {
    let clips = nodes
        .iter()
        .take_while(|node| matches!(node, VectorNode::ClipPath(_)))
        .filter_map(|node| match node {
            // An `ImageVector` group clips with one nonzero path, which is
            // what the lowering gives the drawable target it shares.
            VectorNode::ClipPath(clip) => clip.shapes.first().map(|shape| &shape.data),
            _ => None,
        })
        .collect::<Vec<&PathData>>();
    debug_assert_eq!(
        clips.len(),
        nodes
            .iter()
            .filter(|node| matches!(node, VectorNode::ClipPath(_)))
            .count(),
        "the lowering puts every clip path of a group before its paths"
    );
    let children = nodes.iter().skip(clips.len()).collect();
    (clips, children)
}

/// Kotlin keywords, which cannot name a property or be a package segment.
const KOTLIN_KEYWORDS: &[&str] = &[
    "as",
    "break",
    "class",
    "continue",
    "do",
    "else",
    "false",
    "for",
    "fun",
    "if",
    "in",
    "interface",
    "is",
    "null",
    "object",
    "package",
    "return",
    "super",
    "this",
    "throw",
    "true",
    "try",
    "typealias",
    "typeof",
    "val",
    "var",
    "when",
    "while",
];

/// Whether `name` can name a Kotlin declaration: ASCII letters, digits, and
/// underscores, not starting with a digit, and not a keyword.
pub fn is_kotlin_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
        && name != "_"
        && !KOTLIN_KEYWORDS.contains(&name)
}

/// The Kotlin name an SVG becomes: its
/// [`resource_name`](crate::resource_name) in PascalCase, so `arrow_left` and
/// `Arrow-Left` both become `ArrowLeft`, and `24px icon` becomes `Ic24pxIcon`
/// rather than starting with a digit. Deriving it from the resource name keeps
/// both targets calling the same file the same words. `None` when the name has
/// no letters or digits to keep.
pub fn kotlin_name(stem: &str) -> Option<String> {
    let resource = crate::resource::resource_name(stem)?;
    let mut name = String::with_capacity(resource.len());
    let mut word_start = true;
    for character in resource.chars() {
        if character == '_' {
            word_start = true;
            continue;
        }
        if word_start {
            name.extend(character.to_uppercase());
            word_start = false;
        } else {
            name.push(character);
        }
    }
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_valid_kotlin_identifiers() {
        for (requested, expected) in [
            ("arrow_left", "ArrowLeft"),
            ("Arrow-Left", "ArrowLeft"),
            ("ChevronDown", "ChevronDown"),
            ("HTTPServer", "HttpServer"),
            ("24px Café", "Ic24pxCafe"),
            ("C++", "CPlusPlus"),
            ("class", "IcClass"),
            ("_private", "Private"),
        ] {
            let name = kotlin_name(requested).unwrap();
            assert_eq!(name, expected, "{requested}");
            assert!(is_kotlin_name(&name), "{name}");
        }
        for requested in ["", "_", "🙂", "--"] {
            assert_eq!(kotlin_name(requested), None, "{requested:?}");
        }
    }

    #[test]
    fn keywords_and_malformed_names_are_not_identifiers() {
        for name in [
            "class",
            "when",
            "_",
            "",
            "9lives",
            "arrow-left",
            "arrow left",
        ] {
            assert!(!is_kotlin_name(name), "{name:?}");
        }
        for name in ["ArrowLeft", "_private", "Ic9Lives"] {
            assert!(is_kotlin_name(name), "{name:?}");
        }
    }
}
