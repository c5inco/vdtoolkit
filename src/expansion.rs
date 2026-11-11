//! Bounds what an SVG's references expand to, before anything expands them.
//!
//! usvg builds a referenced element again wherever it is used: a `<use>` copies
//! its target, and a mask, clip path, filter, `<feImage>`, pattern, or marker
//! is converted once for each element that points at it. Content that refers
//! to itself twice per level therefore doubles per level, and a file of a few
//! kilobytes asks for gigabytes: twenty masks that each hold two references to
//! the one before it peaked above 1.6 GB of resident memory. A stylesheet
//! repeats in the same way, since each value of a rule is copied onto every
//! element the rule matches. The limits on what a conversion emits are checked
//! after that tree exists, so they cannot stop it. This pass reads the same
//! references and rules from the parsed XML and adds up what they would build,
//! so the document is rejected while it is still the size of its source.
//!
//! The sum is an upper bound and deliberately not a model of usvg. It ignores
//! the caches that let usvg convert some masks, clip paths, and paint servers
//! once, counts a reference wherever usvg could read one, and takes the
//! costliest element when an `id` is duplicated. Reading too many references
//! only raises the estimate; what must hold is that none usvg follows is
//! missed, so the stylesheet rules are matched by the selector code usvg
//! itself uses.

use std::collections::HashMap;

use crate::error::{Error, Result};

/// Most that references and stylesheet rules may add to a document, in the
/// units of [`weight`]: one per element instance plus one per 32 bytes it
/// carries. usvg peaks at up to about 1 KB of resident memory per unit, so the
/// largest expansion accepted costs about a quarter of a gigabyte. Content the
/// document writes out itself is not counted, so a document with neither is
/// never rejected, and of the 7,690 icons and illustrations this was measured
/// on (Hugeicons, Rune Icons, and the illustration corpus the checkers pin)
/// the most any adds is 1,413.
const MAX_EXPANSION_COST: u64 = 250_000;

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

/// Properties a descendant inherits, in the order of [`Reference::kind`].
/// `marker-mid` is drawn once per path vertex and the others once per element.
const INHERITED: [&str; 5] = ["fill", "stroke", "marker-start", "marker-mid", "marker-end"];
const MARKER_MID: u8 = 3;
/// A reference read on the element that declares it.
const DIRECT: u8 = 5;
/// An `href`, which on a `<use>` also copies the target's elements.
const HREF: u8 = 6;

/// Properties that are not inherited but accept `inherit`, which copies the
/// parent's value and with it a reference this pass cannot place: inside a
/// `<use>` the parent is the element that instantiated the copy.
const INHERITABLE_ON_REQUEST: [&str; 3] = ["clip-path", "mask", "filter"];

/// Elements usvg skips along with everything inside them. It skips every
/// element it has no name for, and only these are listed: a reference read
/// where usvg reads none merely raises the estimate, except that an animation
/// commonly links to the element it sits in, which would read as a cycle.
const UNREAD: [&str; 8] = [
    "style",
    "animate",
    "animateColor",
    "animateMotion",
    "animateTransform",
    "set",
    "mpath",
    "discard",
];

const NONE: u32 = u32::MAX;

/// Rejects a document whose references nest deeper than `max_depth` levels,
/// form a cycle, or together with its stylesheet expand it by more than
/// [`MAX_EXPANSION_COST`].
pub(crate) fn check(document: &roxmltree::Document<'_>, max_depth: usize) -> Result<()> {
    let graph = Graph::new(document);
    let total = graph.evaluate()?;
    if total.depth as usize > max_depth {
        return Err(Error::InvalidInput(format!(
            "SVG references nest its content more than {max_depth} levels deep, counting each reference as a level; flatten the chain of <use>, mask, clip path, filter, pattern, and marker references in the source artwork"
        )));
    }
    if total.cost.saturating_sub(graph.written) > MAX_EXPANSION_COST {
        return Err(Error::InvalidInput(format!(
            "SVG expands to more than {MAX_EXPANSION_COST} elements beyond those it writes out; each <use>, mask, clip path, filter, pattern, or marker reference repeats what it points to, and a stylesheet rule repeats its values on every element it matches"
        )));
    }
    Ok(())
}

/// One thing whose expansion is computed once and shared by all that use it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Item<'a> {
    Element(u32),
    /// Every element carrying this `id`: `<use>` resolves a duplicate to the
    /// first and `url()` to the last, so a reference costs the worst of them.
    Id(&'a str),
    /// What an element inherits for one [`INHERITED`] property from the
    /// element that declares it.
    Paint(u32, u8),
    /// Every reference the document declares for one
    /// [`INHERITABLE_ON_REQUEST`] property.
    Requested(u8),
}

/// How an item's expansion enters the item that depends on it.
#[derive(Clone, Copy)]
enum Role {
    Child,
    Reference,
    /// A `<use>` target, whose elements become elements of the `<use>`.
    Copy,
    Paint(u8),
}

#[derive(Clone, Copy, Default, Debug)]
struct Totals {
    cost: u64,
    /// Elements, counting those `<use>` copies: each resolves inherited paint.
    instances: u64,
    /// Upper bound on path vertices, each of which draws a `marker-mid`.
    vertices: u64,
    /// Levels usvg recurses through, counting the item itself: one for each
    /// element and one more for each reference followed.
    depth: u32,
}

/// One declaration of an attribute, a `style` attribute, or a stylesheet rule,
/// read once however many elements the rule matches.
struct Declared<'a> {
    name: &'a str,
    inherit: bool,
    ids: Vec<&'a str>,
    /// Length of the value.
    bytes: u64,
}

impl<'a> Declared<'a> {
    fn new(name: &'a str, value: &'a str) -> Self {
        Self {
            name,
            inherit: value.trim() == "inherit",
            ids: func_iris(value).collect(),
            bytes: value.len() as u64,
        }
    }
}

struct Reference<'a> {
    element: u32,
    kind: u8,
    id: &'a str,
}

struct Graph<'a, 'input> {
    document: &'a roxmltree::Document<'input>,
    root: u32,
    /// Whether usvg reads the node: an element in the SVG namespace or none
    /// that is not [`UNREAD`], whose ancestors are all read too.
    read: Vec<bool>,
    /// The element whose declaration of each [`INHERITED`] property reaches a
    /// node: itself or the nearest ancestor that declares the property.
    sources: Vec<[u32; INHERITED.len()]>,
    /// Bit `p` is set where a declaration of inherited property `p` says
    /// `inherit`, so the parent's source applies as well; bit `5 + q` where a
    /// declaration of requested property `q` does.
    inherits: Vec<u8>,
    /// In document order, so those of one element are adjacent.
    references: Vec<Reference<'a>>,
    ids: HashMap<&'a str, Vec<u32>>,
    requested: [Vec<&'a str>; INHERITABLE_ON_REQUEST.len()],
    /// What one instance of each element costs to build, in [`weight`] units.
    weights: Vec<u64>,
    /// Cost of the document as written: no reference followed, no rule applied.
    written: u64,
}

impl<'a, 'input> Graph<'a, 'input> {
    fn new(document: &'a roxmltree::Document<'input>) -> Self {
        let count = document.descendants().count();
        let mut graph = Self {
            document,
            root: document.root_element().id().get(),
            read: vec![false; count],
            sources: vec![[NONE; INHERITED.len()]; count],
            inherits: vec![0; count],
            references: Vec::new(),
            ids: HashMap::new(),
            requested: Default::default(),
            weights: vec![0; count],
            written: 0,
        };
        let sheet = style_sheet(document);
        let rules: Vec<Vec<Declared<'a>>> = sheet
            .rules
            .iter()
            .map(|rule| {
                let declarations = rule.declarations.iter();
                declarations
                    .map(|declaration| Declared::new(declaration.name, declaration.value))
                    .collect()
            })
            .collect();
        // `<tref>` copies every character under its target, read or not.
        let mut characters = vec![0u64; count];
        for node in document.descendants().rev() {
            let index = node.id().get_usize();
            if node.is_text() {
                characters[index] = node.text().map_or(0, |text| text.len() as u64);
            }
            if let Some(parent) = node.parent() {
                characters[parent.id().get_usize()] += characters[index];
            }
        }
        // Document order visits a parent before its children.
        for node in document.descendants().filter(roxmltree::Node::is_element) {
            let index = node.id().get();
            if let Some(id) = node.attribute("id") {
                graph.ids.entry(id).or_default().push(index);
            }
            let parent = node.parent().filter(roxmltree::Node::is_element);
            let parent_read = parent.is_none_or(|parent| graph.read[parent.id().get_usize()]);
            let name = node.tag_name();
            if !parent_read
                || !matches!(name.namespace(), None | Some(SVG_NS))
                || UNREAD.contains(&name.name())
            {
                continue;
            }
            graph.read[index as usize] = true;
            let own = attribute_bytes(node)
                + node
                    .children()
                    .filter(roxmltree::Node::is_text)
                    .map(|text| characters[text.id().get_usize()])
                    .sum::<u64>();
            graph.written = graph.written.saturating_add(weight(own));
            if let Some(parent) = parent {
                graph.sources[index as usize] = graph.sources[parent.id().get_usize()];
            }
            let styled = graph.declare(node, &sheet, &rules);
            graph.weights[index as usize] = weight(own.saturating_add(styled));
        }
        for reference in &graph.references {
            if reference.kind != HREF || graph.node(reference.element).tag_name().name() != "tref" {
                continue;
            }
            let copied = graph.ids.get(reference.id).into_iter().flatten();
            let copied = copied.map(|target| characters[*target as usize]).max();
            let weight = &mut graph.weights[reference.element as usize];
            *weight = weight.saturating_add(copied.unwrap_or(0) / BYTES_PER_UNIT);
        }
        for targets in &mut graph.requested {
            targets.sort_unstable();
            targets.dedup();
        }
        graph
    }

    /// Records the references `node` declares, from everywhere usvg collects an
    /// element's properties: its attributes, the stylesheet rules that match
    /// it, and its `style` attribute. Which declaration wins the cascade is not
    /// worked out; all of them count.
    ///
    /// Returns the bytes these add to the element's own: usvg copies each
    /// matching stylesheet value onto the element, and again onto every copy a
    /// `<use>` makes of it.
    fn declare(
        &mut self,
        node: roxmltree::Node<'a, 'input>,
        sheet: &simplecss::StyleSheet<'a>,
        rules: &[Vec<Declared<'a>>],
    ) -> u64 {
        let mut extra = 0u64;
        for attribute in node.attributes() {
            if !matches!(
                attribute.namespace(),
                None | Some(SVG_NS) | Some(XLINK_NS) | Some(XML_NS)
            ) {
                continue;
            }
            match attribute.name() {
                "style" => {
                    for declaration in simplecss::DeclarationTokenizer::from(attribute.value()) {
                        let declared = Declared::new(declaration.name, declaration.value);
                        extra = extra.saturating_add(self.declaration(node, &declared));
                    }
                }
                // usvg turns `<a>` into a group, which has no use for a link.
                "href" if node.tag_name().name() != "a" => {
                    if let Some(id) = iri(attribute.value()) {
                        self.reference(node, HREF, id);
                    }
                }
                name => {
                    let declared = Declared::new(name, attribute.value());
                    extra = extra.saturating_add(self.declaration(node, &declared));
                }
            }
        }
        for (rule, declarations) in sheet.rules.iter().zip(rules) {
            if rule.selector.matches(&Styled(node)) {
                for declared in declarations {
                    extra = extra.saturating_add(self.declaration(node, declared));
                    extra = extra.saturating_add(declared.bytes);
                }
            }
        }
        extra
    }

    /// Records the references of one declaration on `node`, and returns the
    /// bytes it weighs beyond its length.
    fn declaration(&mut self, node: roxmltree::Node<'a, 'input>, declared: &Declared<'a>) -> u64 {
        let index = node.id().get_usize();
        let name = declared.name;
        let inherit = declared.inherit;
        let properties = match name {
            "marker" => 2..5,
            _ => match INHERITED.iter().position(|property| *property == name) {
                Some(property) => property..property + 1,
                None => 0..0,
            },
        };
        if properties.is_empty() {
            let requested = INHERITABLE_ON_REQUEST
                .iter()
                .position(|property| *property == name);
            for id in &declared.ids {
                self.reference(node, DIRECT, id);
                if let Some(property) = requested {
                    self.requested[property].push(id);
                }
            }
            if let (Some(property), true) = (requested, inherit) {
                self.inherits[index] |= 1 << (INHERITED.len() + property);
            }
            return match name {
                "filter" => declared.bytes.saturating_mul(FILTER_SURCHARGE),
                _ => 0,
            };
        }
        for property in properties {
            self.sources[index][property] = index as u32;
            if inherit {
                self.inherits[index] |= 1 << property;
            }
            for id in &declared.ids {
                self.reference(node, property as u8, id);
            }
        }
        0
    }

    fn reference(&mut self, node: roxmltree::Node<'a, 'input>, kind: u8, id: &'a str) {
        self.references.push(Reference {
            element: node.id().get(),
            kind,
            id,
        });
    }

    fn node(&self, index: u32) -> roxmltree::Node<'a, 'input> {
        self.document
            .get_node(roxmltree::NodeId::new(index))
            .expect("the index came from this document")
    }

    fn references(&self, element: u32) -> &[Reference<'a>] {
        let start = self
            .references
            .partition_point(|reference| reference.element < element);
        let end = self
            .references
            .partition_point(|reference| reference.element <= element);
        &self.references[start..end]
    }

    /// The source of inherited property `property` for the parent of `element`.
    fn parent_source(&self, element: u32, property: u8) -> Option<u32> {
        let parent = self.node(element).parent()?;
        let source = self.sources[parent.id().get_usize()][property as usize];
        (source != NONE).then_some(source)
    }

    fn dependencies(&self, item: Item<'a>, mut visit: impl FnMut(Item<'a>, Role)) {
        match item {
            Item::Element(index) => {
                let node = self.node(index);
                for child in node.children() {
                    if self.read[child.id().get_usize()] {
                        visit(Item::Element(child.id().get()), Role::Child);
                    }
                }
                let name = node.tag_name().name();
                for reference in self.references(index) {
                    match (reference.kind, name) {
                        // The characters it copies are part of its own weight.
                        (HREF, "tref") => {}
                        (HREF, "use") => visit(Item::Id(reference.id), Role::Copy),
                        (HREF | DIRECT, _) => visit(Item::Id(reference.id), Role::Reference),
                        _ => {}
                    }
                }
                for property in 0..INHERITABLE_ON_REQUEST.len() as u8 {
                    if self.inherits[index as usize] & (1 << (INHERITED.len() as u8 + property))
                        != 0
                    {
                        visit(Item::Requested(property), Role::Reference);
                    }
                }
                if resolves_paint(node) {
                    for property in 0..INHERITED.len() as u8 {
                        let source = self.sources[index as usize][property as usize];
                        if source != NONE {
                            visit(Item::Paint(source, property), Role::Paint(property));
                        }
                    }
                }
            }
            Item::Id(id) => {
                for element in self.ids.get(id).into_iter().flatten() {
                    if self.read[*element as usize] {
                        visit(Item::Element(*element), Role::Reference);
                    }
                }
            }
            Item::Paint(source, property) => {
                for reference in self.references(source) {
                    if reference.kind == property {
                        visit(Item::Id(reference.id), Role::Reference);
                    }
                }
                if self.inherits[source as usize] & (1 << property) != 0 {
                    if let Some(parent) = self.parent_source(source, property) {
                        visit(Item::Paint(parent, property), Role::Reference);
                    }
                }
            }
            Item::Requested(property) => {
                for id in &self.requested[property as usize] {
                    visit(Item::Id(id), Role::Reference);
                }
            }
        }
    }

    fn combine(&self, item: Item<'a>, totals: impl Fn(Item<'a>) -> Totals) -> Totals {
        let mut total = Totals::default();
        let mut paints = [Totals::default(); INHERITED.len()];
        self.dependencies(item, |dependency, role| {
            let part = totals(dependency);
            if let Role::Paint(property) = role {
                paints[property as usize] = part;
                return;
            }
            total.depth = total.depth.max(part.depth);
            match (item, role) {
                (_, Role::Paint(_)) => {}
                // A reference resolves to one of the elements carrying the id.
                (Item::Id(_), _) => {
                    total.cost = total.cost.max(part.cost);
                    total.instances = total.instances.max(part.instances);
                    total.vertices = total.vertices.max(part.vertices);
                }
                (_, Role::Reference) => total.cost = total.cost.saturating_add(part.cost),
                (_, Role::Child | Role::Copy) => {
                    total.cost = total.cost.saturating_add(part.cost);
                    total.instances = total.instances.saturating_add(part.instances);
                    total.vertices = total.vertices.saturating_add(part.vertices);
                }
            }
        });
        let Item::Element(index) = item else {
            // Following a reference costs usvg more stack than descending to a
            // child does, so it counts as a level of its own.
            if let Item::Id(_) = item {
                total.depth = total.depth.saturating_add(1);
            }
            return total;
        };
        let node = self.node(index);
        total.cost = total.cost.saturating_add(self.weights[index as usize]);
        total.instances = total.instances.saturating_add(1);
        total.vertices = total.vertices.saturating_add(vertices(node));
        // Every element a `<use>` copies inherits from the `<use>`, so what it
        // inherits is resolved once per copy, and below the deepest of them.
        let below = total.depth;
        for (property, paint) in paints.iter().enumerate() {
            let times = if property as u8 == MARKER_MID {
                total.vertices
            } else {
                total.instances
            };
            total.cost = total.cost.saturating_add(times.saturating_mul(paint.cost));
            total.depth = total.depth.max(below.saturating_add(paint.depth));
        }
        total.depth = total.depth.saturating_add(1);
        total
    }

    /// Expansion of the root element, or the error for a reference cycle.
    ///
    /// The walk keeps its own stack: a chain of references is as long as the
    /// document makes it, and recursing along it is the failure this prevents.
    fn evaluate(&self) -> Result<Totals> {
        let mut elements: Vec<Option<Totals>> = vec![None; self.read.len()];
        let mut others: HashMap<Item<'a>, Totals> = HashMap::new();
        let mut open: std::collections::HashSet<Item<'a>> = std::collections::HashSet::new();
        let mut stack = vec![(Item::Element(self.root), false)];
        while let Some((item, expanded)) = stack.pop() {
            let done = |item: Item<'a>| match item {
                Item::Element(index) => elements[index as usize],
                other => others.get(&other).copied(),
            };
            if expanded {
                let total = self.combine(item, |dependency| {
                    done(dependency).expect("dependencies are evaluated first")
                });
                open.remove(&item);
                match item {
                    Item::Element(index) => elements[index as usize] = Some(total),
                    other => {
                        others.insert(other, total);
                    }
                }
                continue;
            }
            if done(item).is_some() {
                continue;
            }
            // An item still open when it is reached again is being expanded by
            // one of its own dependencies. usvg skips the shortest such cycles
            // and overflows its stack on the rest.
            if !open.insert(item) {
                return Err(Error::InvalidInput(format!(
                    "SVG references form a cycle through {}; an element cannot be drawn with a reference that leads back to it",
                    self.describe(item)
                )));
            }
            stack.push((item, true));
            self.dependencies(item, |dependency, _| {
                if done(dependency).is_none() {
                    stack.push((dependency, false));
                }
            });
        }
        Ok(elements[self.root as usize].expect("the root was evaluated"))
    }

    fn describe(&self, item: Item<'a>) -> String {
        match item {
            Item::Id(id) => format!("#{id}"),
            Item::Element(index) | Item::Paint(index, _) => {
                let node = self.node(index);
                match node.attribute("id") {
                    Some(id) => format!("#{id}"),
                    None => format!("<{}>", node.tag_name().name()),
                }
            }
            Item::Requested(property) => {
                format!("{}=\"inherit\"", INHERITABLE_ON_REQUEST[property as usize])
            }
        }
    }
}

/// Bytes of an element's text that cost as much as the element itself.
const BYTES_PER_UNIT: u64 = 32;

/// What one instance of an element costs to build: itself, and the `bytes` of
/// its attributes, character data, and matching stylesheet values, since path
/// data is parsed and the rest stored again for every copy.
fn weight(bytes: u64) -> u64 {
    1 + bytes / BYTES_PER_UNIT
}

/// Bytes of the attributes usvg stores or parses again for every copy. A link
/// is not among them: an `href` names its target or carries an embedded image,
/// which is decoded and dropped, so a `<use>` of an image costs no more than
/// the element.
fn attribute_bytes(node: roxmltree::Node<'_, '_>) -> u64 {
    node.attributes()
        .filter(|attribute| attribute.name() != "href")
        .map(|attribute| attribute.value().len() as u64)
        .sum()
}

/// Extra weight of each byte of a `filter` value. Every function in the list
/// becomes a filter of its own, about sixty times its eight bytes, and usvg
/// takes time in the square of how many a document ends up with.
const FILTER_SURCHARGE: u64 = 7;

/// Upper bound on the vertices of a shape: path data spends at least a byte on
/// each segment it draws.
fn vertices(node: roxmltree::Node<'_, '_>) -> u64 {
    match node.tag_name().name() {
        "path" | "line" | "polyline" | "polygon" | "rect" | "circle" | "ellipse" => {
            2 + attribute_bytes(node)
        }
        _ => 0,
    }
}

/// Whether usvg resolves fill, stroke, and markers for the element: shapes and
/// text, and `<use>` on behalf of what it copies. A group passes them on to
/// its children, and a gradient or pattern has none of its own.
fn resolves_paint(node: roxmltree::Node<'_, '_>) -> bool {
    matches!(
        node.tag_name().name(),
        "path"
            | "line"
            | "polyline"
            | "polygon"
            | "rect"
            | "circle"
            | "ellipse"
            | "text"
            | "tspan"
            | "tref"
            | "textPath"
            | "use"
    )
}

/// The stylesheet usvg applies: every `<style>` that is CSS, in document order.
fn style_sheet<'a>(document: &'a roxmltree::Document<'_>) -> simplecss::StyleSheet<'a> {
    let mut sheet = simplecss::StyleSheet::new();
    for node in document.descendants().filter(|n| n.has_tag_name("style")) {
        if !matches!(node.attribute("type"), None | Some("text/css")) {
            continue;
        }
        if let Some(text) = node.text() {
            sheet.parse_more(text);
        }
    }
    sheet
}

/// An element as usvg presents it to the selector matcher.
struct Styled<'a, 'input>(roxmltree::Node<'a, 'input>);

impl simplecss::Element for Styled<'_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element().map(Styled)
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling_element().map(Styled)
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        self.0.tag_name().name() == local_name
    }

    fn attribute_matches(&self, local_name: &str, operator: simplecss::AttributeOperator) -> bool {
        self.0
            .attribute(local_name)
            .is_some_and(|value| operator.matches(value))
    }

    fn pseudo_class_matches(&self, class: simplecss::PseudoClass) -> bool {
        match class {
            simplecss::PseudoClass::FirstChild => self.prev_sibling_element().is_none(),
            _ => false,
        }
    }
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// The id an `href` links to, read as svgtypes reads an IRI.
fn iri(value: &str) -> Option<&str> {
    let link = value.trim_start_matches(|c: char| c.is_ascii() && is_space(c as u8));
    let link = link.strip_prefix('#')?;
    let link = link.split(' ').next().unwrap_or(link);
    (!link.is_empty()).then_some(link)
}

/// The ids of every `url(#id)` in a property value, each read as svgtypes
/// reads a functional IRI. A paint or mask has one and a filter list several;
/// reading all of them from any value costs nothing when there are none.
fn func_iris(value: &str) -> impl Iterator<Item = &str> {
    let bytes = value.as_bytes();
    let mut position = 0;
    std::iter::from_fn(move || {
        while position + 4 <= bytes.len() {
            let start = position;
            position += 1;
            if bytes[start..start + 4].eq_ignore_ascii_case(b"url(") {
                if let Some(id) = func_iri(&value[start + 4..]) {
                    return Some(id);
                }
            }
        }
        None
    })
}

/// The id in what follows `url(`.
fn func_iri(rest: &str) -> Option<&str> {
    let skip = |text: &str| -> usize {
        text.bytes()
            .position(|byte| !is_space(byte))
            .unwrap_or(text.len())
    };
    let mut rest = &rest[skip(rest)..];
    let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"'));
    if quote.is_some() {
        rest = &rest[1..];
        rest = &rest[skip(rest)..];
    }
    let rest = rest.strip_prefix('#')?;
    let link = match quote {
        Some(quote) => rest[..rest.find(quote)?].trim_end(),
        None => &rest[..rest.find([' ', ')']).unwrap_or(rest.len())],
    };
    (!link.is_empty()).then_some(link)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_links_the_way_the_parser_does() {
        assert_eq!(iri("#a"), Some("a"));
        assert_eq!(iri("  #a b"), Some("a"));
        assert_eq!(iri("a"), None);
        assert_eq!(iri("#"), None);
        let ids = |value| func_iris(value).collect::<Vec<_>>();
        assert_eq!(ids("url(#a)"), ["a"]);
        assert_eq!(ids(" url( '#a b' ) red"), ["a b"]);
        assert_eq!(ids("url(\"#a\")"), ["a"]);
        assert_eq!(ids("url(#a) blur(2) url(#b)"), ["a", "b"]);
        assert_eq!(ids("URL(#a)"), ["a"]);
        assert_eq!(ids("url(a.svg#b)"), [] as [&str; 0]);
        assert_eq!(ids("#url"), [] as [&str; 0]);
    }
}
