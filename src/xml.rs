use std::fmt::Write;

use crate::vector::{
    Color, FillRule, GradientStop, LineCap, LineJoin, Paint, PathCommand, TileMode, VectorDrawable,
    VectorNode, VectorPath,
};

pub fn write(drawable: &VectorDrawable) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    let _ = writeln!(
        xml,
        "<vector xmlns:android=\"http://schemas.android.com/apk/res/android\""
    );
    if drawable.uses_gradients() {
        let _ = writeln!(xml, "    xmlns:aapt=\"http://schemas.android.com/aapt\"");
    }
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
    let mut gradients = Vec::new();
    if let Some(paint) = &path.fill {
        match paint {
            Paint::Solid(color) => {
                let _ = writeln!(
                    xml,
                    "{attribute_indent}android:fillColor=\"{}\"",
                    rgb(*color)
                );
            }
            gradient => gradients.push(("android:fillColor", gradient)),
        }
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
    if let Some(paint) = &path.stroke {
        match paint {
            Paint::Solid(color) => {
                let _ = writeln!(
                    xml,
                    "{attribute_indent}android:strokeColor=\"{}\"",
                    rgb(*color)
                );
            }
            gradient => gradients.push(("android:strokeColor", gradient)),
        }
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
    if gradients.is_empty() {
        let _ = writeln!(xml, "{attribute_indent}/>");
        return;
    }
    let _ = writeln!(xml, "{attribute_indent}>");
    for (name, gradient) in gradients {
        write_gradient(xml, name, gradient, attribute_indent);
    }
    let _ = writeln!(xml, "{indent}</path>");
}

/// Write an inline complex color, which AAPT2 extracts into a color resource.
fn write_gradient(xml: &mut String, name: &str, paint: &Paint, indent: &str) {
    let element_indent = format!("{indent}    ");
    let attribute_indent = format!("{element_indent}    ");
    let _ = writeln!(xml, "{indent}<aapt:attr name=\"{name}\">");
    let _ = writeln!(xml, "{element_indent}<gradient");
    let (stops, tile_mode): (&[GradientStop], TileMode) = match paint {
        Paint::Linear(gradient) => {
            let _ = writeln!(xml, "{attribute_indent}android:type=\"linear\"");
            let _ = writeln!(
                xml,
                "{attribute_indent}android:startX=\"{}\"",
                number(gradient.start_x)
            );
            let _ = writeln!(
                xml,
                "{attribute_indent}android:startY=\"{}\"",
                number(gradient.start_y)
            );
            let _ = writeln!(
                xml,
                "{attribute_indent}android:endX=\"{}\"",
                number(gradient.end_x)
            );
            let _ = writeln!(
                xml,
                "{attribute_indent}android:endY=\"{}\"",
                number(gradient.end_y)
            );
            (&gradient.stops, gradient.tile_mode)
        }
        Paint::Radial(gradient) => {
            let _ = writeln!(xml, "{attribute_indent}android:type=\"radial\"");
            let _ = writeln!(
                xml,
                "{attribute_indent}android:centerX=\"{}\"",
                number(gradient.center_x)
            );
            let _ = writeln!(
                xml,
                "{attribute_indent}android:centerY=\"{}\"",
                number(gradient.center_y)
            );
            let _ = writeln!(
                xml,
                "{attribute_indent}android:gradientRadius=\"{}\"",
                number(gradient.radius)
            );
            (&gradient.stops, gradient.tile_mode)
        }
        Paint::Solid(_) => unreachable!("solid paint is written as an attribute"),
    };
    match tile_mode {
        TileMode::Clamp => {}
        TileMode::Mirror => {
            let _ = writeln!(xml, "{attribute_indent}android:tileMode=\"mirror\"");
        }
        TileMode::Repeat => {
            let _ = writeln!(xml, "{attribute_indent}android:tileMode=\"repeat\"");
        }
    }
    let _ = writeln!(xml, "{attribute_indent}>");
    for stop in stops {
        let _ = writeln!(
            xml,
            "{attribute_indent}<item android:offset=\"{}\" android:color=\"{}\"/>",
            number(stop.offset),
            argb(stop.color, stop.alpha)
        );
    }
    let _ = writeln!(xml, "{element_indent}</gradient>");
    let _ = writeln!(xml, "{indent}</aapt:attr>");
}

fn rgb(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.0, color.1, color.2)
}

fn argb(color: Color, alpha: f32) -> String {
    let alpha = crate::vector::alpha_byte(alpha);
    if alpha == u8::MAX {
        rgb(color)
    } else {
        format!("#{alpha:02X}{:02X}{:02X}{:02X}", color.0, color.1, color.2)
    }
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
