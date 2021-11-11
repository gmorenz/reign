use super::{var_attr_regex, Code, Error, Parse, ParseStream, Tokenize, ViewFields};
use proc_macro2::{Span, TokenStream};
use quote::{quote, TokenStreamExt};
use syn::LitStr;

#[derive(Debug)]
pub struct VariableAttribute {
    pub name: String,
    pub value: Code,
}

impl Parse for VariableAttribute {
    fn parse(input: &mut ParseStream) -> Result<Self, Error> {
        Ok(VariableAttribute {
            name: input.capture(&var_attr_regex(), 1)?,
            value: Code::parse_expr(input)?,
        })
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
