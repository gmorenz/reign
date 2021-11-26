pub mod parse; // TODO(gmorenz): Does this need pub?
pub mod tokenize; // TODO(gmorenz): Does this need pub?

// Parsing and tokenizing for the rust language, we don't really need to
// "understand" this code in the same way, so I haven't spent much time refactoring
// it. Would love it if we could shell out this part to libraries more.
pub mod expr;
pub mod pat;

use std::fmt::{Debug, Formatter, Error as FError};

use syn::Member;

use self::{expr::Expr, pat::For};

#[derive(Debug)]
pub struct ItemTemplate {
    pub name: String,
    pub attrs: Vec<Attribute>,
    pub children: Vec<Node>,
}

#[derive(Debug)]
pub enum Node {
    Element(Element),
    Comment(Comment),
    Text(Text),
    Doctype(Doctype),
}

#[derive(Debug)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<Attribute>,
    pub children: Vec<Node>,
}

#[derive(Debug)]
pub struct Text {
    pub content: Vec<StringPart>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Attribute {
    Normal(NormalAttribute),
    Dynamic(DynamicAttribute),
    Variable(VariableAttribute),
    Control(ControlAttribute),
}

#[derive(Debug)]
pub struct NormalAttribute {
    pub name: String,
    pub value: AttributeValue,
}

#[derive(Debug)]
pub struct AttributeValue {
    pub parts: Vec<StringPart>,
}

#[derive(Debug)]
pub struct DynamicAttribute {
    pub symbol: String,
    pub prefix: String,
    pub name: Code,
    pub suffix: String,
    pub value: Code,
}

#[derive(Debug)]
pub struct VariableAttribute {
    pub name: String,
    pub value: Code,
}

#[derive(Debug)]
pub struct ControlAttribute {
    pub name: String,
    pub value: Code,
}

#[derive(Debug)]
pub struct Doctype {
    pub content: String,
}

#[derive(Debug)]
pub struct Comment {
    pub content: String,
}

pub enum Code {
    For(For),
    Expr(Expr),
}

#[derive(Debug)]
pub enum StringPart {
    Normal(String),
    Expr(Code),
}

impl Debug for Code {
    fn fmt(&self, _: &mut Formatter) -> Result<(), FError> {
        Ok(())
    }
}



// Utils

fn is_member_named(member: &Member) -> bool {
    match member {
        Member::Named(_) => true,
        Member::Unnamed(_) => false,
    }
}
