use std::fmt::Write;

use crate::vector::{
    Color, FillRule, GradientStop, LineCap, LineJoin, Paint, PathCommand, TileMode, VectorDrawable,
    VectorNode, VectorPath,
};

/// Serialize a drawable. `short_paths` spells path data in its shortest form,
/// which `Asset::optimize` turns on once numbers are rounded.
pub fn write(drawable: &VectorDrawable, short_paths: bool) -> String {
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
    write_nodes(&mut xml, &drawable.children, 1, short_paths);
    xml.push_str("</vector>\n");
    xml
}

/// Remove formatting whitespace that appears only between XML tags or
/// attributes. VectorDrawable XML contains no text nodes, so this preserves
/// its parsed structure and every attribute value.
pub fn compact(text: &str) -> String {
    let mut compact = String::with_capacity(text.len());
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if compact.chars().last().is_some_and(|last| {
            last != '>' && !matches!(line.as_bytes().first(), Some(b'>') | Some(b'/'))
        }) {
            compact.push(' ');
        }
        compact.push_str(line);
    }
    compact
}

fn write_nodes(xml: &mut String, nodes: &[VectorNode], depth: usize, short_paths: bool) {
    let indent = "    ".repeat(depth);
    let attribute_indent = "    ".repeat(depth + 1);
    for node in nodes {
        match node {
            VectorNode::Group(group) => {
                if group.is_identity() {
                    let _ = writeln!(xml, "{indent}<group>");
                } else {
                    let _ = write!(xml, "{indent}<group");
                    if group.pivot_x != 0.0 {
                        let _ = write!(
                            xml,
                            "\n{attribute_indent}android:pivotX=\"{}\"",
                            number(group.pivot_x)
                        );
                    }
                    if group.pivot_y != 0.0 {
                        let _ = write!(
                            xml,
                            "\n{attribute_indent}android:pivotY=\"{}\"",
                            number(group.pivot_y)
                        );
                    }
                    if group.rotation != 0.0 {
                        let _ = write!(
                            xml,
                            "\n{attribute_indent}android:rotation=\"{}\"",
                            number(group.rotation)
                        );
                    }
                    if group.scale_x != 1.0 {
                        let _ = write!(
                            xml,
                            "\n{attribute_indent}android:scaleX=\"{}\"",
                            number(group.scale_x)
                        );
                    }
                    if group.scale_y != 1.0 {
                        let _ = write!(
                            xml,
                            "\n{attribute_indent}android:scaleY=\"{}\"",
                            number(group.scale_y)
                        );
                    }
                    if group.translate_x != 0.0 {
                        let _ = write!(
                            xml,
                            "\n{attribute_indent}android:translateX=\"{}\"",
                            number(group.translate_x)
                        );
                    }
                    if group.translate_y != 0.0 {
                        let _ = write!(
                            xml,
                            "\n{attribute_indent}android:translateY=\"{}\"",
                            number(group.translate_y)
                        );
                    }
                    let _ = writeln!(xml, ">");
                }
                write_nodes(xml, &group.children, depth + 1, short_paths);
                let _ = writeln!(xml, "{indent}</group>");
            }
            VectorNode::ClipPath(clip) => {
                // A drawable's clip path is one path filled by the nonzero
                // rule, which is what the lowering gives it. A clip holding no
                // shape writes nothing rather than panicking; the lowering
                // never makes one.
                let Some(shape) = clip.shapes.first() else {
                    continue;
                };
                let _ = writeln!(xml, "{indent}<clip-path");
                let _ = writeln!(
                    xml,
                    "{attribute_indent}android:pathData=\"{}\"",
                    path_text(&shape.data.0, short_paths)
                );
                let _ = writeln!(xml, "{attribute_indent}/>");
            }
            VectorNode::Path(path) => {
                write_path(xml, path, &indent, &attribute_indent, short_paths)
            }
        }
    }
}

fn write_path(
    xml: &mut String,
    path: &VectorPath,
    indent: &str,
    attribute_indent: &str,
    short_paths: bool,
) {
    let _ = writeln!(xml, "{indent}<path");
    let _ = writeln!(
        xml,
        "{attribute_indent}android:pathData=\"{}\"",
        path_text(&path.path_data.0, short_paths)
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

fn path_text(commands: &[PathCommand], short_paths: bool) -> String {
    if short_paths {
        crate::short_path::write(commands)
    } else {
        path_data(commands)
    }
}

/// Every command absolute with its letter, one space between commands.
pub(crate) fn path_data(commands: &[PathCommand]) -> String {
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

/// Format a number with at most six decimals.
///
/// Six fixed decimals can spell out binary noise for an `f32` above about 64,
/// where the spacing between adjacent values is coarser than a millionth:
/// `107.64` would print as `107.639999`. When the shortest decimal that reads
/// back as the same `f32` is shorter, it is used instead. Both texts denote
/// the same value, so this only drops digits that carry no information.
pub(crate) fn number(value: f32) -> String {
    use std::io::{Cursor, Write as _};

    let value = if value.abs() < 0.000_000_5 {
        0.0
    } else {
        value
    };
    // An f32 has at most 24 significant bits; 1_000_000 = 2^6 * 15_625
    // adds at most 14 more. The f64 product is exact, so ties-to-even gives
    // exactly the integer digits of std's six-decimal formatting.
    let scaled = (f64::from(value) * 1_000_000.0).round_ties_even();
    // Even -f32::MAX with six decimals fits in 47 bytes.
    let mut digits = [0; 64];
    let text = if scaled.abs() < 9.0e15 {
        // The bound keeps the integer cast in range, including after rounding.
        let scaled = scaled.abs() as u64;
        let mut whole = scaled / 1_000_000;
        let mut fraction = scaled % 1_000_000;
        let mut start = digits.len();
        let mut end = start;
        if fraction != 0 {
            for _ in 0..6 {
                start -= 1;
                digits[start] = b'0' + (fraction % 10) as u8;
                fraction /= 10;
            }
            while digits[end - 1] == b'0' {
                end -= 1;
            }
            start -= 1;
            digits[start] = b'.';
        }
        loop {
            start -= 1;
            digits[start] = b'0' + (whole % 10) as u8;
            whole /= 10;
            if whole == 0 {
                break;
            }
        }
        if value.is_sign_negative() {
            start -= 1;
            digits[start] = b'-';
        }
        std::str::from_utf8(&digits[start..end]).expect("number digits are ASCII")
    } else {
        // Preserve std's spelling for large values, infinities, and NaNs.
        let mut fixed = Cursor::new(digits.as_mut_slice());
        write!(fixed, "{value:.6}").expect("fixed f32 fits in the buffer");
        let end = fixed.position() as usize;
        std::str::from_utf8(&digits[..end])
            .expect("formatted number is ASCII")
            .trim_end_matches('0')
            .trim_end_matches('.')
    };
    // Below 16, half an ULP is at most 2^-21 < 0.000_000_5. Any shorter
    // round-tripping decimal has at most six places and must therefore equal
    // the rounded fixed text. Only larger values need a second formatting.
    if value.abs() >= 16.0 {
        // At 16 and above, Display writes at most 40 bytes.
        let mut buffer = [0; 64];
        let mut shortest = Cursor::new(buffer.as_mut_slice());
        write!(shortest, "{value}").expect("displayed f32 fits in the buffer");
        let end = shortest.position() as usize;
        let shortest = std::str::from_utf8(&buffer[..end]).expect("formatted number is ASCII");
        if shortest.len() < text.len() && !shortest.contains(['e', 'E']) {
            return shortest.to_owned();
        }
    }
    text.to_owned()
}

#[cfg(test)]
mod tests {
    use super::number;

    // Keep the original formatter as an independent, byte-for-byte oracle.
    fn original_number(value: f32) -> String {
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
        let shortest = value.to_string();
        if shortest.len() < text.len() && !shortest.contains(['e', 'E']) {
            shortest
        } else {
            text
        }
    }

    #[test]
    fn number_matches_original_formatter() {
        let check = |value: f32| {
            assert_eq!(
                number(value),
                original_number(value),
                "value {value:?}, bits {:#010x}",
                value.to_bits()
            );
        };
        let check_both_signs = |value: f32| {
            check(value);
            check(-value);
        };

        // Every exponent, with mantissas at both ends and in the middle:
        // includes signed zero, subnormals, finite extremes, infinities and
        // signaling/quiet NaNs of both signs and multiple payloads.
        for exponent in 0..=255 {
            for mantissa in [0, 1, 2, 0x3f_ffff, 0x7f_fffe, 0x7f_ffff] {
                check_both_signs(f32::from_bits((exponent << 23) | mantissa));
            }
        }
        // Exercise both sides of normalization, the shortest-form cutoff,
        // decimal carries and the integer-formatting fallback boundary.
        for value in [0.000_000_5_f32, 1.0, 10.0, 16.0, 64.0, 100.0, 9.0e9] {
            for bits in value.to_bits() - 256..=value.to_bits() + 256 {
                check_both_signs(f32::from_bits(bits));
            }
        }
        // Odd multiples of 1/128 have an exact seventh-decimal tie. Check
        // both ties-to-even directions and their immediate f32 neighbors.
        for numerator in (1..131_072).step_by(2) {
            let tie = numerator as f32 / 128.0;
            for bits in tie.to_bits() - 1..=tie.to_bits() + 1 {
                check_both_signs(f32::from_bits(bits));
            }
        }
        // Decimal inputs exercise leading/trailing fractional zeros, carries,
        // and values where Display is shorter than the six-decimal spelling.
        for decimals in 0..=7 {
            let scale = 10_f64.powi(decimals);
            for numerator in -10_000..=10_000 {
                check((f64::from(numerator) / scale) as f32);
            }
        }
        // Reproducible random signs and mantissas, with exponents concentrated
        // on the digit path. Earlier sections cover zeroing and std fallback.
        let mut bits = 0x6d2b_79f5_u32;
        for _ in 0..500_000 {
            bits ^= bits << 13;
            bits ^= bits >> 17;
            bits ^= bits << 5;
            let exponent = 105 + (bits >> 23 & 0xff) % 57;
            check(f32::from_bits(bits & 0x807f_ffff | exponent << 23));
        }
    }
}
