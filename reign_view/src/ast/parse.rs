use super::*;

mod parse_stream;
mod error;
pub mod consts;

use consts::*;
use error::Error;
use parse_stream::ParseStream;
use regex::Regex;
use syn::parse_str;

pub fn parse(data: String, template_name: String) -> Result<ItemTemplate, Error> {
    let mut ps = ParseStream::new(data);
    let item = ItemTemplate::parse(&mut ps, template_name)?;

    ps.skip_spaces()?;

    if ps.content.len() != ps.cursor {
        // TODO: Remove this restriction
        Err(ps.error("only one top-level node is allowed"))
    } else {
        Ok(item)
    }
}

trait Parse: Sized {
    fn parse(input: &mut ParseStream) -> Result<Self, Error>;
}

impl ItemTemplate {
    fn parse(input: &mut ParseStream, name: String) -> Result<Self, Error> {
        let mut template = None;
        let mut style = None;

        for _ in 0.. 2 {
            let tag_name = input.capture(&tag_name_regex(), 1)?;

            if tag_name == "template" {
                if template.is_some() {
                    return Err(input.error("Expected a single 'template' element"))
                }

                template = Some((
                    parse_element_attrs(input)?,
                    parse_element_children(input, "template")?,
                ))
            }
            else if tag_name == "style" {
                if style.is_some() {
                    return Err(input.error("Expected a single 'style' element"))
                }

                style = Some(parse_style_element(input)?);
            }
            else {
                return Err(input.error("Expected 'template' or 'style' element, found something else"));
            };
        }

        let (attrs, children) = template.ok_or(
            input.error("Missing 'template' element"))?;

        Ok(ItemTemplate {
            name,
            attrs,
            children,
            style: style.unwrap_or_default(),
        })
    }
}

// Currently just grabs contents, really we should properly understand stylesheets...
fn parse_style_element(input: &mut ParseStream) -> Result<String, Error> {
    let attrs = parse_element_attrs(input)?;
    if attrs.len() != 0 {
        return Err(input.error("Attributes on 'style' elements unsupported"))
    }

    if input.peek("/>") {
        return Err(input.error("Self closing style tags unsupported (sorry)"))
    }

    input.step(">")?;

    // TODO: Robust tag parsing (things like </ input>)
    let out = input.until("</input>", true)?;
    input.step("</input>")?;
    Ok(out)
}

fn parse_element_attrs(input: &mut ParseStream) -> Result<Vec<Attribute>, Error> {
    let mut attrs = vec![];
    input.skip_spaces()?;

    while !input.peek("/>") && !input.peek(">") {
        attrs.push(input.parse()?);
        input.skip_spaces()?;
    }

    Ok(attrs)
}

fn parse_element_children(input: &mut ParseStream, tag_name: &str) -> Result<Vec<Node>, Error> {
    let mut children = vec![];

    if input.peek("/>") {
        input.step("/>")?;
    } else {
        // input.peek(">") is true here
        input.step(">")?;

        // TODO:(view:html) Tags that can be left open according to HTML spec
        if !VOID_TAGS.contains(&tag_name) {
            let closing_tag = format!("</{}", tag_name);

            while !input.peek(&closing_tag) {
                let child = input.parse()?;
                children.push(child);
            }

            input.step(&closing_tag)?;
            input.skip_spaces()?;
            input.step(">")?;
        }
    }

    Ok(children)
}

impl Parse for Node {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        if input.cursor == 0 {
            input.skip_spaces()?;
        }

        if input.peek("<!--") {
            Ok(Node::Comment(input.parse()?))
        } else if input.is_match(DOCTYPE) {
            Ok(Node::Doctype(input.parse()?))
        } else if input.is_match(&tag_name_regex()) {
            Ok(Node::Element(input.parse()?))
        } else {
            let text: Text = input.parse()?;

            if text.content.is_empty() {
                return Err(input.error("unable to continue parsing"));
            }

            Ok(Node::Text(text))
        }
    }
}

fn tag_name_regex() -> String {
    format!("<({0}(:?:{0})*)", consts::TAG_NAME)
}

impl Parse for Element {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        let name = input.capture(&tag_name_regex(), 1)?;

        Ok(Element {
            name: name.to_lowercase(),
            attrs: parse_element_attrs(input)?,
            children: parse_element_children(input, &name)?,
        })
    }
}

impl Parse for Text {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        Ok(Text {
            content: input.parse_text()?,
        })
    }
}

impl Parse for Attribute {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        if input.is_match(&dy_attr_regex()) {
            Ok(Attribute::Dynamic(input.parse()?))
        } else if input.is_match(&var_attr_regex()) {
            Ok(Attribute::Variable(input.parse()?))
        } else if input.is_match(CTRL_ATTR) {
            Ok(Attribute::Control(input.parse()?))
        } else if input.is_match(ATTR_NAME) {
            Ok(Attribute::Normal(input.parse()?))
        } else {
            Err(input.error("unable to parse attribute"))
        }
    }
}

fn dy_attr_regex() -> String {
    format!(
        "{}{2}{}{2}",
        VAR_ATTR_SYMBOL, DY_ATTR_EXPR, DY_ATTR_NAME_PART
    )
}

fn var_attr_regex() -> String {
    format!("{}({})", VAR_ATTR_SYMBOL, ATTR_NAME)
}


impl Parse for NormalAttribute {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        Ok(NormalAttribute {
            name: input.matched(ATTR_NAME)?,
            value: input.parse()?,
        })
    }
}

impl Parse for AttributeValue {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        Ok(AttributeValue {
            parts: {
                let value = AttributeValue::parse_to_str(input)?;
                StringPart::parse(input, &value, true)?
            },
        })
    }
}

impl AttributeValue {
    pub fn parse_to_str(input: &mut ParseStream) -> Result<String, Error> {
        input.skip_spaces()?;

        if input.peek("=") {
            input.step("=")?;
            input.skip_spaces()?;

            if input.peek("\"") {
                Ok(input.capture(ATTR_VALUE_DOUBLE_QUOTED, 1)?)
            } else if input.peek("\'") {
                Ok(input.capture(ATTR_VALUE_SINGLE_QUOTED, 1)?)
            } else {
                Ok(input.matched(ATTR_VALUE_UNQUOTED)?)
            }
        } else {
            Ok("\"\"".to_string())
        }
    }
}

impl Parse for DynamicAttribute {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        Ok(DynamicAttribute {
            symbol: input.step(":")?,
            prefix: input.matched(DY_ATTR_NAME_PART)?,
            name: {
                let name = input.capture(DY_ATTR_EXPR, 1)?;
                Code::parse_expr_from_str(input, &name)?
            },
            suffix: input.matched(DY_ATTR_NAME_PART)?,
            value: Code::parse_expr(input)?,
        })
    }
}

impl Parse for VariableAttribute {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        Ok(VariableAttribute {
            name: input.capture(&var_attr_regex(), 1)?,
            value: Code::parse_expr(input)?,
        })
    }
}

impl Parse for ControlAttribute {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        let name = input.capture(CTRL_ATTR, 1)?;

        Ok(ControlAttribute {
            name: name.clone(),
            value: {
                if name == "for" {
                    Code::parse_for(input)?
                } else {
                    Code::parse_expr(input)?
                }
            },
        })
    }
}

impl Parse for Doctype {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        Ok(Doctype {
            content: input.matched(DOCTYPE)?,
        })
    }
}

impl Parse for Comment {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        input.step("<!--")?;

        Ok(Comment {
            content: input.until("-->", true)?,
        })
    }
}

impl Code {
    pub fn parse_for(input: &mut ParseStream) -> Result<Self, Error> {
        let string = AttributeValue::parse_to_str(input)?;
        Self::parse_for_from_str(input, &string)
    }

    pub fn parse_expr(input: &mut ParseStream) -> Result<Self, Error> {
        let string = AttributeValue::parse_to_str(input)?;
        Self::parse_expr_from_str(input, &string)
    }

    pub fn parse_for_from_str(input: &mut ParseStream, text: &str) -> Result<Self, Error> {
        let parsed = parse_str::<For>(text);

        if let Ok(code) = parsed {
            Ok(Code::For(code))
        } else {
            Err(input.error("expected pattern in expression"))
        }
    }

    pub fn parse_expr_from_str(input: &ParseStream, text: &str) -> Result<Self, Error> {
        let parsed = parse_str::<Expr>(text);

        if let Ok(code) = parsed {
            Ok(Code::Expr(code))
        } else {
            Err(input.error("expected expression"))
        }
    }
}

impl StringPart {
    pub fn parse(input: &mut ParseStream, data: &str, in_attr: bool) -> Result<Vec<Self>, Error> {
        let mut parts = vec![];
        let start_regex = Regex::new(r"\\\{\{|\{\{|<").unwrap();
        let mut cursor = if !in_attr { input.cursor } else { 0 };

        loop {
            let remaining = data.get(cursor..).unwrap();

            if remaining.is_empty() {
                break;
            }

            let matches = start_regex.find(remaining);

            if matches.is_none() {
                parts.push(StringPart::Normal(remaining.to_string()));
                cursor += remaining.len();
                break;
            }

            let until = cursor + matches.unwrap().start();
            let sub_string = data.get(cursor..until).unwrap();

            if !sub_string.is_empty() {
                parts.push(StringPart::Normal(sub_string.to_string()));
                cursor = until;
            }

            match data.get(cursor..=cursor).unwrap() {
                "\\" => {
                    parts.push(StringPart::Normal("\\{{".to_string()));
                    cursor += 3;
                }
                "<" => {
                    if in_attr {
                        parts.push(StringPart::Normal("<".to_string()));
                        cursor += 1;
                    } else {
                        break;
                    }
                }
                "{" => {
                    cursor += 2;
                    let end_remaining = data.get(cursor..).unwrap();
                    let end_matches = end_remaining.find("}}");

                    if end_matches.is_none() {
                        if !in_attr {
                            input.cursor = cursor;
                        }

                        return Err(input.error("expression incomplete"));
                    }

                    let expr_until = cursor + end_matches.unwrap();
                    let expr_string = data.get(cursor..expr_until).unwrap();

                    parts.push(StringPart::Expr(Code::parse_expr_from_str(
                        input,
                        expr_string,
                    )?));
                    cursor = expr_until + 2;
                }
                _ => unreachable!(),
            }
        }

        if !in_attr {
            input.cursor = cursor;
        }

        Ok(parts)
    }
}