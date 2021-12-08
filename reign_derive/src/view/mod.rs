mod ast;


use std::{fs::read_to_string, io::Error, collections::HashMap, env, path::{Path, PathBuf}};

use inflector::cases::pascalcase::to_pascal_case;
use once_cell::sync::OnceCell;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::abort_call_site;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_str,
    punctuated::Punctuated,
    token::{Colon2, Comma},
    Expr, Ident, LitStr,
};
use once_cell::sync::Lazy;
use regex::Regex;


use self::ast::{parse::parse, tokenize::tokenize};
use crate::{utils::Options, INTERNAL_ERR};

static IDENTMAP: OnceCell<HashMap<String, Vec<(String, bool)>>> = OnceCell::new();

// TODO: derive: Options after the paths (including changing `crate::views`)
// Can't use parse_separated_non_empty here
pub struct Views {
    paths: Punctuated<LitStr, Comma>,
}

impl Parse for Views {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Views {
            paths: input.parse_terminated(|i| i.parse::<LitStr>())?,
        })
    }
}

pub struct Render {
    path: Punctuated<Ident, Colon2>,
    options: Options,
}

impl Render {
    fn id(&self) -> String {
        self.parts().join(":")
    }

    fn parts(&self) -> Vec<String> {
        self.path.iter().map(|i| format!("{}", i)).collect()
    }
}

impl Parse for Render {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Render {
            path: Punctuated::<Ident, Colon2>::parse_separated_nonempty(input)?,
            options: input.parse()?,
        })
    }
}

fn get_dir(input: Views) -> PathBuf {
    let mut dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    for i in input.paths.into_iter() {
        dir.push(i.value());
    }

    dir
}

pub fn views(input: Views) -> TokenStream {
    let dir = get_dir(input);
    let mut map = HashMap::new();

    let mut i = 0;
    let views = recurse(
        &dir,
        "",
        &mut map,
        &mut |ident, files| {
            Ok(quote! {
                pub mod #ident {
                    #(#files)*
                }
            })
        },
        &mut |dir: &Path, file_name: &str, file_code: TokenStream| {
            // Incldue source as a string so that rustc knows it needs
            // to run this again when the source code changes.

            // Get an (absolute) path to the source
            let mut file_path = dir.to_path_buf();
            file_path.push(file_name);
            let file_path = file_path.into_os_string().into_string().unwrap();

            // Create a name for the constant
            let source_name = format!("_SOURCE_{}", i);
            let source_ident = syn::Ident::new(&source_name, Span::call_site());
            i += 1;

            // Workaround to quoting the include_str macro instead of literally
            // including the file into the quote.
            let include_str_ident = syn::Ident::new("include_str", Span::call_site());

            Ok(quote! {
                const #source_ident: &str = #include_str_ident !(#file_path);
                #file_code
            })
        },
        &mut |_, views| Ok(views),
    )
    .unwrap();

    IDENTMAP.set(map).expect(INTERNAL_ERR);

    quote! {
        pub mod views {
            #(#views)*
        }
    }
}

fn view_path(input: &Render) -> TokenStream {
    let parts = input.parts();
    let (last, elements) = parts.split_last().unwrap();

    let view = Ident::new(&to_pascal_case(last), Span::call_site());
    let path: Vec<Ident> = elements
        .iter()
        .map(|x| Ident::new(x, Span::call_site()))
        .collect();

    quote! {
        #(#path::)*#view
    }
}

fn capture(input: &Render) -> TokenStream {
    let path = view_path(input);
    let value = IDENTMAP.get().expect(INTERNAL_ERR).get(&input.id());

    if value.is_none() {
        abort_call_site!("expected a path referencing to a view file");
    }

    let idents: Vec<TokenStream> = value
        .expect(INTERNAL_ERR)
        .iter()
        .map(|x| {
            let ident = Ident::new(&x.0, Span::call_site());

            let rest = if x.1 {
                quote! {}
            } else {
                quote! {
                    : #ident.as_ref()
                }
            };

            quote! {
                #ident#rest
            }
        })
        .collect();

    quote! {
        crate::views::#path {
            #(#idents),*
        }
    }
}

pub fn render(mut input: Render) -> TokenStream {
    let capture = capture(&input);

    let status: Expr = input
        .options
        .remove("status")
        .unwrap_or_else(|| parse_str("200").unwrap());

    if cfg!(feature = "router") {
        quote! {
            ::reign::router::helpers::render(#capture, #status)
        }
    } else {
        quote! {
            format!("{}", #capture)
        }
    }
}




pub type Manifest = HashMap<String, Vec<(String, bool)>>;

pub static FILE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([[:alpha:]]([[:word:]]*[[:alnum:]])?)\.html$").expect(INTERNAL_ERR)
});
pub static FOLDER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([[:alpha:]]([[:word:]]*[[:alnum:]])?)").expect(INTERNAL_ERR));

pub fn recurse<O, I, P>(
    path: &Path,
    relative_path: &str,
    manifest: &mut Manifest,
    folder_hook: &mut O,
    file_hook: &mut I,
    path_hook: &mut P,
) -> Result<Vec<TokenStream>, Error>
where
    O: FnMut(Ident, Vec<TokenStream>) -> Result<TokenStream, Error>,
    I: FnMut(&Path, &str, TokenStream) -> Result<TokenStream, Error>,
    P: FnMut(&Path, Vec<TokenStream>) -> Result<Vec<TokenStream>, Error>,
{
    let mut views = vec![];

    for entry in path.read_dir().expect(INTERNAL_ERR) {
        if let Ok(entry) = entry {
            let new_path = entry.path();
            let file_name_os_str = entry.file_name();
            let file_name = file_name_os_str.to_string_lossy();

            if new_path.is_dir() {
                if !FOLDER_REGEX.is_match(&file_name) {
                    continue;
                }

                let ident = Ident::new(&file_name, Span::call_site());
                let sub_relative_path = format!("{}:{}", relative_path, file_name);

                let sub_views = recurse(
                    &new_path,
                    &sub_relative_path,
                    manifest,
                    folder_hook,
                    file_hook,
                    path_hook,
                )?;

                views.push(folder_hook(ident, sub_views)?);
                continue;
            }

            if !FILE_REGEX.is_match(&file_name) {
                continue;
            }

            let file_base_name = file_name.trim_end_matches(".html");
            let template_name = to_pascal_case(file_base_name);
            let data = read_to_string(new_path)?
                .replace("\r\n", "\n");
            // TODO: Error reporting improvements
            let template_item = parse(data, template_name).expect("Failed to parse template");
            let (file_view, idents) = tokenize(&template_item);

            // let (file_view, idents) = tokenize_view(&new_path, file_base_name);

            let file_key = format!("{}:{}", relative_path, file_base_name)
                .trim_start_matches(':')
                .to_string();

            manifest.insert(
                file_key,
                idents.iter().map(|x| (format!("{}", x.0), x.1)).collect(),
            );

            views.push(file_hook(path, &file_name, file_view)?);
        }
    }

    Ok(path_hook(path, views)?)
}
