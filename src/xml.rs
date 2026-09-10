use std::fmt::Write;

use crate::vector::{
    FillRule, LineCap, LineJoin, PathCommand, VectorDrawable, VectorNode, VectorPath,
};

pub fn write(drawable: &VectorDrawable) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    let _ = writeln!(
        xml,
        "<vector xmlns:android=\"http://schemas.android.com/apk/res/android\""
    );
    let _ = writeln!(xml, "    android:width=\"{}dp\"", number(drawable.width_dp));
    let _ = writeln!(
        xml,
        "    android:height=\"{}dp\"",
        number(drawable.height_dp)
    );
    let _ = writeln!(
        xml,
        "    android:viewportWidth=\"{}\"",
        number(drawable.viewport_width)
    );
    let _ = writeln!(
        xml,
        "    android:viewportHeight=\"{}\">",
        number(drawable.viewport_height)
    );
    write_nodes(&mut xml, &drawable.children, 1);
    xml.push_str("</vector>\n");
    xml
}

fn write_nodes(xml: &mut String, nodes: &[VectorNode], depth: usize) {
    let indent = "    ".repeat(depth);
    let attribute_indent = "    ".repeat(depth + 1);
    for node in nodes {
        match node {
            VectorNode::Group(group) => {
                let _ = writeln!(xml, "{indent}<group>");
                write_nodes(xml, &group.children, depth + 1);
                let _ = writeln!(xml, "{indent}</group>");
            }
            VectorNode::ClipPath(path_data_value) => {
                let _ = writeln!(xml, "{indent}<clip-path");
                let _ = writeln!(
                    xml,
                    "{attribute_indent}android:pathData=\"{}\"",
                    path_data(&path_data_value.0)
                );
                let _ = writeln!(xml, "{attribute_indent}/>");
            }
            VectorNode::Path(path) => write_path(xml, path, &indent, &attribute_indent),
        }
    }
}

fn write_path(xml: &mut String, path: &VectorPath, indent: &str, attribute_indent: &str) {
    let _ = writeln!(xml, "{indent}<path");
    let _ = writeln!(
        xml,
        "{attribute_indent}android:pathData=\"{}\"",
        path_data(&path.path_data.0)
    );
    if let Some(color) = path.fill {
        let _ = writeln!(
            xml,
            "{attribute_indent}android:fillColor=\"#{:02X}{:02X}{:02X}\"",
            color.0, color.1, color.2
        );
        if path.fill_alpha != 1.0 {
            let _ = writeln!(
                xml,
                "{attribute_indent}android:fillAlpha=\"{}\"",
                number(path.fill_alpha)
            );
        }
        if path.fill_rule == FillRule::EvenOdd {
            let _ = writeln!(xml, "{attribute_indent}android:fillType=\"evenOdd\"");
        }
    }
    if let Some(color) = path.stroke {
        let _ = writeln!(
            xml,
            "{attribute_indent}android:strokeColor=\"#{:02X}{:02X}{:02X}\"",
            color.0, color.1, color.2
        );
        let _ = writeln!(
            xml,
            "{attribute_indent}android:strokeWidth=\"{}\"",
            number(path.stroke_width)
        );
        if path.stroke_alpha != 1.0 {
            let _ = writeln!(
                xml,
                "{attribute_indent}android:strokeAlpha=\"{}\"",
                number(path.stroke_alpha)
            );
        }
        if path.stroke_cap != LineCap::Butt {
            let value = match path.stroke_cap {
                LineCap::Butt => unreachable!(),
                LineCap::Round => "round",
                LineCap::Square => "square",
            };
            let _ = writeln!(xml, "{attribute_indent}android:strokeLineCap=\"{value}\"");
        }
        if path.stroke_join != LineJoin::Miter {
            let value = match path.stroke_join {
                LineJoin::Miter => unreachable!(),
                LineJoin::Round => "round",
                LineJoin::Bevel => "bevel",
            };
            let _ = writeln!(xml, "{attribute_indent}android:strokeLineJoin=\"{value}\"");
        }
        if path.stroke_miter != 4.0 {
            let _ = writeln!(
                xml,
                "{attribute_indent}android:strokeMiterLimit=\"{}\"",
                number(path.stroke_miter)
            );
        }
    }
    let _ = writeln!(xml, "{attribute_indent}/>");
}

fn path_data(commands: &[PathCommand]) -> String {
    let mut value = String::new();
    for command in commands {
        if !value.is_empty() {
            value.push(' ');
        }
        match *command {
            PathCommand::Move(x, y) => write!(value, "M{},{}", number(x), number(y)),
            PathCommand::Line(x, y) => write!(value, "L{},{}", number(x), number(y)),
            PathCommand::Quad(a, b, x, y) => write!(
                value,
                "Q{},{} {},{}",
                number(a),
                number(b),
                number(x),
                number(y)
            ),
            PathCommand::Cubic(a, b, c, d, x, y) => write!(
                value,
                "C{},{} {},{} {},{}",
                number(a),
                number(b),
                number(c),
                number(d),
                number(x),
                number(y)
            ),
            PathCommand::Close => {
                value.push('Z');
                Ok(())
            }
        }
        .expect("writing to a string cannot fail");
    }
    value
}

fn number(value: f32) -> String {
    let value = if value.abs() < 0.000_000_5 {
        0.0
    } else {
        value
    };
    let mut text = format!("{value:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}
