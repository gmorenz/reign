use super::super::consts::*;
use super::{AttributeValue, Error, Parse, ParseStream, Tokenize, ViewFields};
use proc_macro2::{Span, TokenStream};
use quote::{quote, TokenStreamExt};
use syn::LitStr;

#[derive(Debug)]
pub struct NormalAttribute {
    pub name: String,
    pub value: AttributeValue,
}

impl Parse for NormalAttribute {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        Ok(NormalAttribute {
            name: input.matched(ATTR_NAME)?,
            value: input.parse()?,
        })
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
