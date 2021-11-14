use crate::parse::string_part::tokenize_string_parts;

use super::{Error, Parse, ParseStream, StringPart, Tokenize, ViewFields};
use proc_macro2::TokenStream;
use quote::{quote, TokenStreamExt};

#[derive(Debug)]
pub struct Text {
    pub content: Vec<StringPart>,
}

impl Parse for Text {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        Ok(Text {
            content: input.parse_text()?,
        })
    }
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

// Based on https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html#rule-1-html-encode-before-inserting-untrusted-data-into-html-element-content
pub fn encode_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&#x27;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}