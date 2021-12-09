mod ast;


use std::{fs::read_to_string, io::Error, env, path::{Path, PathBuf}};

use inflector::cases::pascalcase::to_pascal_case;
// use proc_macro::{Ident, Span};
use proc_macro2::{Span, TokenStream};
use quote::{quote, TokenStreamExt};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
    Ident, LitStr,
};
use once_cell::sync::Lazy;
use regex::Regex;


use self::ast::{parse::parse, tokenize::tokenize, ItemTemplate};
use crate::INTERNAL_ERR;

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

fn get_dir(input: Views) -> PathBuf {
    let mut dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    for i in input.paths.into_iter() {
        dir.push(i.value());
    }

    dir
}

pub (crate) fn views(input: Views) -> TokenStream {
    let dir = get_dir(input);

    let mut i = 0;

    let mut templates = vec![];
    collect_views(&dir, &mut templates).unwrap();

    let modules = templates.into_iter().map(|(path, template)| {
        let (file_view, _idents) = tokenize(&template);

        // Incldue source as a string so that rustc knows it needs
        // to run this again when the source code changes.

        let path_str = path.clone().into_os_string().into_string().unwrap();

        // Create a name for the constant
        let source_name = format!("_SOURCE_{}", i);
        let source_ident = syn::Ident::new(&source_name, Span::call_site());
        i += 1;

        // Workaround to quoting the include_str macro instead of literally
        // including the file into the quote.
        let include_str_ident = syn::Ident::new("include_str", Span::call_site());

        let relative_path = path.strip_prefix(&dir).unwrap().to_owned();
        (relative_path, quote! {
            const #source_ident: &str = #include_str_ident !(#path_str);
            #file_view
        })
    }).collect::<Vec<_>>();

    let output = build_mod_tree(&modules);

    quote! {
        pub mod views {
            #output
        }
    }
}

/// Takes input, in the order of a depth first search, with a list of paths
/// relative to the root views folder, and coverts it into a module tree.
fn build_mod_tree(mut input: &[(PathBuf, TokenStream)]) -> TokenStream {
    build_mod_tree_recurse(Path::new(""), &mut input)
}

fn build_mod_tree_recurse(root: &Path, input: &mut &[(PathBuf, TokenStream)]) -> TokenStream {
    let mut out = TokenStream::new(); // TODO: Potentially pass this down for efficiency?

    loop {
        if input.len() == 0 {
            // We're done
            break
        }
        let (path, tokens) = &input[0];
        if !path.starts_with(root) {
            // We're no longer in this module
            break
        }

        let parent = path.parent().unwrap();
        if parent != root {
            // this is a sub module

            // TODO: Only recurse one path componenet at a time (this breaks if there are empty folders)
            let tokens = build_mod_tree_recurse(parent, input);
            let mod_name = parent.file_name().unwrap().to_str().expect("Non-utf8 file/dir name");
            let mod_ident = Ident::new(mod_name, Span::call_site());

            out.append_all(quote! {
                pub mod #mod_ident {
                    #tokens
                }
            });

            continue
        }

        // In this module
        *input = &input[1..];
        out.append_all(tokens.clone());
    }

    out
}

fn collect_views(
    path: &Path,
    // Recursive function requires mutable output collector...
    out: &mut Vec<(PathBuf, ItemTemplate)>,
) -> Result<(), Error> {
    for entry in path.read_dir().expect(INTERNAL_ERR) {
        if let Ok(entry) = entry {
            let new_path = entry.path();
            let file_name_os_str = entry.file_name();
            let file_name: &str = &*file_name_os_str.to_string_lossy();

            if new_path.is_dir() {
                if !FOLDER_REGEX.is_match(&file_name) {
                    continue;
                }

                collect_views(
                    &new_path,
                    out,
                )?;

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

            let mut view_path = path.to_path_buf();
            view_path.push(file_name);
            out.push((view_path, template_item));
        }
    }

    Ok(())
}

static FILE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([[:alpha:]]([[:word:]]*[[:alnum:]])?)\.html$").expect(INTERNAL_ERR)
});
static FOLDER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([[:alpha:]]([[:word:]]*[[:alnum:]])?)").expect(INTERNAL_ERR));

// fn recurse<O, I, P>(
//     path: &Path,
//     relative_path: &str,
//     manifest: &mut HashMap<String, ItemTemplate>,
//     folder_hook: &mut O,
//     file_hook: &mut I,
//     path_hook: &mut P,
// ) -> Result<Vec<TokenStream>, Error>
// where
//     O: FnMut(Ident, Vec<TokenStream>) -> Result<TokenStream, Error>,
//     I: FnMut(&Path, &str, TokenStream) -> Result<TokenStream, Error>,
//     P: FnMut(&Path, Vec<TokenStream>) -> Result<Vec<TokenStream>, Error>,
// {
//     let mut views = vec![];

//     for entry in path.read_dir().expect(INTERNAL_ERR) {
//         if let Ok(entry) = entry {
//             let new_path = entry.path();
//             let file_name_os_str = entry.file_name();
//             let file_name = file_name_os_str.to_string_lossy();

//             if new_path.is_dir() {
//                 if !FOLDER_REGEX.is_match(&file_name) {
//                     continue;
//                 }

//                 let ident = Ident::new(&file_name, Span::call_site());
//                 let sub_relative_path = format!("{}:{}", relative_path, file_name);

//                 let sub_views = recurse(
//                     &new_path,
//                     &sub_relative_path,
//                     manifest,
//                     folder_hook,
//                     file_hook,
//                     path_hook,
//                 )?;

//                 views.push(folder_hook(ident, sub_views)?);
//                 continue;
//             }

//             if !FILE_REGEX.is_match(&file_name) {
//                 continue;
//             }

//             let file_base_name = file_name.trim_end_matches(".html");
//             let template_name = to_pascal_case(file_base_name);
//             let data = read_to_string(new_path)?
//                 .replace("\r\n", "\n");
//             // TODO: Error reporting improvements
//             let template_item = parse(data, template_name).expect("Failed to parse template");
//             let (file_view, idents) = tokenize(&template_item);

//             // let (file_view, idents) = tokenize_view(&new_path, file_base_name);

//             let file_key = format!("{}:{}", relative_path, file_base_name)
//                 .trim_start_matches(':')
//                 .to_string();

//             manifest.insert(
//                 file_key,
//                 idents.iter().map(|x| (format!("{}", x.0), x.1)).collect(),
//             );

//             views.push(file_hook(path, &file_name, file_view)?);
//         }
//     }

//     Ok(path_hook(path, views)?)
// }
