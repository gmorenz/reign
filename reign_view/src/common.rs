use crate::{INTERNAL_ERR, ast::{parse::parse, tokenize::tokenize}};
use inflector::cases::pascalcase::to_pascal_case;
use once_cell::sync::Lazy;
use proc_macro2::{Ident, Span, TokenStream};
use regex::Regex;
use std::{collections::HashMap, fs::read_to_string, io::Error, path::Path};

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
