#![doc(html_logo_url = "https://reign.rs/images/media/reign.png")]
#![doc(html_root_url = "https://docs.rs/reign_derive/0.2.1")]
#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use syn::parse_macro_input;

#[cfg(feature = "view")]
mod view;

#[cfg(feature = "view")]
mod utils;

pub(crate) const INTERNAL_ERR: &str =
    "Internal error on reign_derive. Please create an issue on https://github.com/pksunkara/reign";

/// Auto load the views from the given directory.
///
/// Folder names should start with an alphabet and end with alphanumeric
/// with underscores being allowed in the middle.
///
/// File names should start with an alphabet and end with alphanumeric
/// with underscores being allowed in the middle. The only allowed
/// extension is `.html`.
///
/// Ignores the other files and folders which do not adhere the above rules.
///
/// Both the folder and file names are converted to lower case before
/// building the template.
///
/// # Examples
///
/// ```ignore
/// use reign::prelude::*;
///
/// views!("src", "views");
/// ```
#[cfg(feature = "view")]
#[proc_macro]
pub fn views(input: TokenStream) -> TokenStream {
    let input: view::Views = parse_macro_input!(input);

    view::views(input).into()
}
