use inflector::cases::{pascalcase::to_pascal_case, snakecase::to_snake_case};
use proc_macro2::{Punct, Spacing, Span, TokenStream};
use quote::{ToTokens, TokenStreamExt, quote};
use syn::{Ident, LitStr, punctuated::{Pair, Punctuated}};

use super::*;

pub(crate) use view_fields::ViewFields;

mod view_fields;

pub fn tokenize(template: &ItemTemplate) -> (TokenStream, Vec<(Ident, bool)>) {
    let template_name = Ident::new(&template.name, Span::call_site());

    let mut fmt_tokens = TokenStream::new();
    let mut idents = ViewFields::new();

    {
        let scopes = ViewFields::new();

        // Generate attrs so they can be used for type ascription,
        // but don't actually emit the code since we just throw it away.
        let _attrs = attrs_tokens(&template.attrs, &mut idents, &scopes);


        // Template.tokenize(tokens, idents, scopes)
        let children = nodes_tokens(&template.children, &mut idents, &scopes);

        // TODO: We aren't considering top level if/for directives, forbid them.
        fmt_tokens.append_all(
            quote! {
                #(#children)*
            }
        )
    }

    let (idents, types) = (idents.keys(), idents.values());

    let new_idents: Vec<Ident> = idents.iter().map(|x| x.0.clone()).collect();

    (
        quote! {
            pub struct #template_name<'a> {
                #(pub #new_idents: #types),*
            }

            #[allow(unused_variables)]
            impl<'a> std::fmt::Display for #template_name<'a> {
                fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    #fmt_tokens
                    Ok(())
                }
            }
        },
        idents,
    )
}

pub(crate) trait Tokenize {
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields);
}


impl Tokenize for Node {
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        match self {
            Node::Element(e) => e.tokenize(tokens, idents, scopes),
            Node::Comment(c) => c.tokenize(tokens, idents, scopes),
            Node::Text(t) => t.tokenize(tokens, idents, scopes),
            Node::Doctype(d) => d.tokenize(tokens, idents, scopes),
        };
    }
}

impl Tokenize for Element {
    #[allow(clippy::cognitive_complexity)]
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        let tag_pieces: Vec<&str> = self.name.split(':').collect();
        let mut new_scopes = scopes.clone();

        // Check for loop to see what variables are defined for this loop (`scopes`)
        if let Some(attr_for) = self.control_attr("for") {
            if let Code::For(for_) = &attr_for.value {
                new_scopes.append(for_.declared());
            }
        }

        let mut elem = if self.name == "template" {
            let children = nodes_tokens(&self.children, idents, &new_scopes);

            quote! {
                #(#children)*
            }
        } else if tag_pieces.len() == 1 && is_reserved_tag(&self.name) {
            let start_tag = LitStr::new(&format!("<{}", &self.name), Span::call_site());
            let attrs = attrs_tokens(&self.attrs, idents, &new_scopes);
            let children = nodes_tokens(&self.children, idents, &new_scopes);
            let end_tokens = self.end_tokens();

            quote! {
                write!(f, "{}", #start_tag)?;
                #(#attrs)*
                write!(f, ">")?;
                #(#children)*
                #end_tokens
            }
        } else {
            let path = convert_tag_name(tag_pieces);
            let attrs = self.component_attrs(idents, &new_scopes);

            // TODO: Deal with children when we deal with slots...

            quote! {
                write!(f, "{}", crate::views::#(#path)::* {
                    #(#attrs),*
                })?;
            }
        };

        elem = if let Some(r_for) = self.control_attr("for") {
            // For loop
            let mut for_expr = TokenStream::new();
            r_for.value.tokenize(&mut for_expr, idents, scopes);

            quote! {
                for #for_expr {
                    #elem
                }
            }
        } else if let Some(r_if) = self.control_attr("if") {
            // If condition
            let mut if_expr = TokenStream::new();
            r_if.value.tokenize(&mut if_expr, idents, scopes);

            quote! {
                if #if_expr {
                    #elem
                }
            }
        } else if let Some(r_else_if) = self.control_attr("else-if") {
            // Else If condition
            let mut if_expr = TokenStream::new();
            r_else_if.value.tokenize(&mut if_expr, idents, scopes);

            quote! {
                else if #if_expr {
                    #elem
                }
            }
        } else if self.control_attr("else").is_some() {
            // Else condition
            quote! {
                else {
                    #elem
                }
            }
        } else {
            elem
        };

        tokens.append_all(elem);
    }
}

impl Element {
    fn control_attr(&self, name: &str) -> Option<&ControlAttribute> {
        for attr in &self.attrs {
            if let Attribute::Control(control) = attr {
                if control.name == name {
                    return Some(control);
                }
            }
        }

        None
    }

    fn template_name(&self) -> Option<String> {
        if self.name == "template" {
            for attr in &self.attrs {
                if let Attribute::Normal(n) = attr {
                    if n.name.starts_with('#') {
                        return Some(n.name.clone());
                    }
                }
            }
        }

        None
    }

    // TODO: Build a DAG out of the views, and use default() if the attrs are not defined
    // It would be even better if we could compile each html file into `.rs` file and use
    // it to speed up compile times.
    //
    // If we have the DAG, and we see an html file was changed, we rebuild that view, and
    // if any of it fields have changed, we need to go up in the DAG and recompile all the
    // views that depend on this.
    //
    // After having DAG, we can also look into intelligently forwarding the types of the
    // view fields into each components.
    fn component_attrs(&self, idents: &mut ViewFields, scopes: &ViewFields) -> Vec<TokenStream> {
        let mut attrs = vec![];

        for attr in &self.attrs {
            let mut tokens = TokenStream::new();

            match attr {
                Attribute::Normal(n) => {
                    tokens.append(Ident::new(&to_snake_case(&n.name), Span::call_site()));
                    tokens.append(Punct::new(':', Spacing::Alone));
                    n.value.tokenize(&mut tokens, idents, scopes);
                }
                Attribute::Variable(v) => {
                    tokens.append(Ident::new(&to_snake_case(&v.name), Span::call_site()));
                    tokens.append(Punct::new(':', Spacing::Alone));
                    v.value.tokenize(&mut tokens, idents, scopes);
                }
                _ => continue,
            }

            attrs.push(tokens);
        }

        attrs
    }

    fn end_tokens(&self) -> TokenStream {
        use super::parse::consts::VOID_TAGS;
        if !VOID_TAGS.contains(&self.name.as_str()) {
            let end_tag = LitStr::new(&format!("</{}>", &self.name), Span::call_site());

            quote! {
                write!(f, "{}", #end_tag)?;
            }
        } else {
            quote! {}
        }
    }
}

fn attrs_tokens(attrs: &[Attribute], idents: &mut ViewFields, scopes: &ViewFields) -> Vec<TokenStream> {
    attrs
        .iter()
        .map(|x| {
            let mut ts = TokenStream::new();

            x.tokenize(&mut ts, idents, &scopes);
            ts
        })
        .collect()
}

fn nodes_tokens(nodes: &[Node], idents: &mut ViewFields, scopes: &ViewFields) -> Vec<TokenStream> {
    let mut tokens = vec![];
    let mut iter = nodes.iter();
    let mut child_option = iter.next();

    while child_option.is_some() {
        let child = child_option.unwrap();

        if let Node::Element(e) = child {
            if e.template_name().is_some() {
                child_option = iter.next();
                continue;
            }

            if e.control_attr("if").is_some() {
                let mut after_if = vec![child];
                let mut next = iter.next();
                let (mut has_else, mut has_else_if) = (false, false);

                while next.is_some() {
                    let sibling = next.unwrap();

                    if let Node::Element(e) = sibling {
                        if e.template_name().is_some() {
                            next = iter.next();
                            continue;
                        }

                        // If element has `else`, Mark the children to be cleaned
                        if e.control_attr("else").is_some() {
                            after_if.push(sibling);
                            has_else = true;
                            child_option = iter.next();
                            break;
                        }

                        // If element has `else-if`, mark the children to be cleaned even though we have no `else`
                        if e.control_attr("else-if").is_some() {
                            has_else_if = true;
                        } else {
                            // Otherwise go to the main loop
                            child_option = next;
                            break;
                        }
                    }

                    after_if.push(sibling);
                    next = iter.next();
                }

                after_if = clean_if_else_group(after_if, has_else, has_else_if);

                for i in after_if {
                    let mut ts = TokenStream::new();

                    i.tokenize(&mut ts, idents, scopes);
                    tokens.push(ts);
                }

                // If at the end, break out
                if next.is_none() {
                    break;
                }

                continue;
            }

            if e.control_attr("else").is_some() || e.control_attr("else-if").is_some() {
                // TODO:(view:err) Show the error position
                panic!("expected `!if` element before `!else` or `!else-if`");
            }
        }

        let mut ts = TokenStream::new();

        child.tokenize(&mut ts, idents, scopes);
        tokens.push(ts);
        child_option = iter.next();
    }

    tokens
}


fn clean_if_else_group(group: Vec<&Node>, has_else: bool, has_else_if: bool) -> Vec<&Node> {
    if has_else {
        // Clean completely
        group
            .into_iter()
            .filter(|x| {
                if let Node::Element(_) = x {
                    true
                } else {
                    false
                }
            })
            .collect()
    } else if has_else_if {
        // Clean only between if and else_if
        let mut last_element = group
            .iter()
            .rev()
            .position(|x| {
                if let Node::Element(_) = x {
                    true
                } else {
                    false
                }
            })
            .unwrap();

        last_element = group.len() - last_element - 1;

        group
            .into_iter()
            .enumerate()
            .filter(|(i, x)| {
                if *i > last_element {
                    return true;
                }

                if let Node::Element(_) = x {
                    true
                } else {
                    false
                }
            })
            .map(|(_, x)| x)
            .collect()
    } else {
        group
    }
}

fn convert_tag_name(tag: Vec<&str>) -> Vec<Ident> {
    let mut idents: Vec<Ident> = tag
        .into_iter()
        .map(|t| Ident::new(&to_snake_case(t), Span::call_site()))
        .collect();

    if let Some(ident) = idents.pop() {
        let new_ident = to_pascal_case(&ident.to_string());
        idents.push(Ident::new(&new_ident, Span::call_site()));
    }

    idents
}

fn is_reserved_tag(tag: &str) -> bool {
    use super::parse::consts::{SVG_TAGS, HTML_TAGS};
    SVG_TAGS.contains(&tag) || HTML_TAGS.contains(&tag)
}

impl Tokenize for Text {
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        let mut ts = TokenStream::new();
        tokenize_string_parts(&self.content, &mut ts, idents, scopes, |input_stream| quote!{
            ::reign::view::encode_text(&#input_stream)
        });
        // self.content.tokenize(&mut ts, idents, scopes);

        tokens.append_all(quote! {
            f.write_str(&format!(#ts))?;
        })
    }
}

impl Tokenize for Attribute {
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        match self {
            Attribute::Normal(n) => n.tokenize(tokens, idents, scopes),
            Attribute::Dynamic(d) => d.tokenize(tokens, idents, scopes),
            Attribute::Variable(v) => v.tokenize(tokens, idents, scopes),
            _ => {}
        };
    }
}

impl Tokenize for NormalAttribute {
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        let name = LitStr::new(&self.name, Span::call_site());
        let mut value = TokenStream::new();

        self.value.tokenize(&mut value, idents, scopes);

        tokens.append_all(quote! {
            let value = ::reign::view::encode_attribute_data(&format!("{}", #value));
            write!(f, " {}={}", #name, value)?;
        });
    }
}

impl Tokenize for AttributeValue {
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        if !self.has_expr() {
            let mut string = self.value().unwrap();

            if string == "\"\"" {
                string = "".to_string();
            }

            // TODO:(view:html-escape)
            let value = LitStr::new(&string, Span::call_site());

            tokens.append_all(quote! { #value });
        } else {
            let mut ts = TokenStream::new();
            // TODO (gmorenz): This definitely needs to escape things... I thought I had already done that. Now I'm worried.
            tokenize_string_parts(&self.parts, &mut ts, idents, scopes, |input_tokens| input_tokens);

            // eprintln!("{:?}", ts);

            tokens.append_all(quote! {
                format!(#ts)
            })
        }
    }
}

impl AttributeValue {
    fn value(&self) -> Option<String> {
        let mut strings: Vec<String> = vec![];

        for part in &self.parts {
            if let StringPart::Normal(s) = part {
                strings.push(s.clone());
            } else {
                return None;
            }
        }

        Some(strings.join(""))
    }

    fn has_expr(&self) -> bool {
        for part in &self.parts {
            if let StringPart::Expr(_) = part {
                return true;
            }
        }

        false
    }
}

impl Tokenize for DynamicAttribute {
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        let prefix = LitStr::new(&self.prefix, Span::call_site());
        let suffix = LitStr::new(&self.suffix, Span::call_site());
        let mut name = TokenStream::new();
        let mut value = TokenStream::new();

        self.name.tokenize(&mut name, idents, scopes);
        self.value.tokenize(&mut value, idents, scopes);

        tokens.append_all(quote! {
            let value = ::reign::view::encode_attribute_data(&format!("{}", #value));
            write!(f, " {}{}{}={}", #prefix, #name, #suffix, value)?;
        });
    }
}

impl Tokenize for VariableAttribute {
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        let name = LitStr::new(&self.name, Span::call_site());
        let mut value = TokenStream::new();

        self.value.tokenize(&mut value, idents, scopes);

        tokens.append_all(quote! {
            let value = ::reign::view::encode_attribute_data(&format!("{}", #value));
            write!(f, " {}={}", #name, value)?;
        });
    }
}

impl Tokenize for Doctype {
    fn tokenize(&self, tokens: &mut TokenStream, _: &mut ViewFields, _: &ViewFields) {
        let doctype_str = LitStr::new(&self.content, Span::call_site());

        tokens.append_all(quote! {
            write!(f, "{}", #doctype_str)?;
        });
    }
}

impl Tokenize for Comment {
    fn tokenize(&self, tokens: &mut TokenStream, _: &mut ViewFields, _: &ViewFields) {
        let content = format!("<!--{}-->", self.content);
        let comment_str = LitStr::new(&content, Span::call_site());

        tokens.append_all(quote! {
            write!(f, "{}", #comment_str)?;
        });
    }
}


impl Tokenize for Code {
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        match self {
            Code::For(f) => f.tokenize(tokens, idents, scopes),
            Code::Expr(e) => e.tokenize(tokens, idents, scopes),
        }
    }
}

impl StringPart {
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields, map_tokens: fn(TokenStream) -> TokenStream) {
        match self {
            StringPart::Normal(n) => {
                let lit = LitStr::new(&n, Span::call_site());
                lit.to_tokens(tokens);
            }
            // TODO:(view:html-escape) expression
            StringPart::Expr(e) => {
                let mut expr_tokens = TokenStream::new();
                e.tokenize(&mut expr_tokens, idents, scopes);
                TokenStreamExt::append_all(tokens, map_tokens(expr_tokens));
            }
        }
    }
}

pub fn tokenize_string_parts(this: &[StringPart], tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields, map_tokens: fn(TokenStream) -> TokenStream) {
    let format_arg_str = "{}".repeat(this.len());
    let format_arg_lit = LitStr::new(&format_arg_str, Span::call_site());

    let content: Vec<TokenStream> = this
        .iter()
        .map(|x| {
            let mut ts = TokenStream::new();

            x.tokenize(&mut ts, idents, scopes, map_tokens);
            ts
        })
        .collect();

    tokens.append_all(quote! {
        #format_arg_lit, #(#content),*
    });
}

impl<T, P> Tokenize for Punctuated<T, P>
where
    T: Tokenize,
    P: ToTokens,
{
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        let mut iter = self.pairs();

        loop {
            let item = iter.next();

            if item.is_none() {
                break;
            }

            match item.unwrap() {
                Pair::Punctuated(t, p) => {
                    t.tokenize(tokens, idents, scopes);
                    p.to_tokens(tokens);
                }
                Pair::End(t) => t.tokenize(tokens, idents, scopes),
            }
        }
    }
}

impl<T> Tokenize for Option<Box<T>>
where
    T: Tokenize,
{
    fn tokenize(&self, tokens: &mut TokenStream, idents: &mut ViewFields, scopes: &ViewFields) {
        if self.is_some() {
            self.as_ref().unwrap().tokenize(tokens, idents, scopes);
        }
    }
}
