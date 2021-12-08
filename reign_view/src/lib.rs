#![doc(html_logo_url = "https://reign.rs/images/media/reign.png")]
#![doc(html_root_url = "https://docs.rs/reign_view/0.2.1")]
#![doc = include_str!("../README.md")]

#[doc(hidden)]
pub use maplit;

#[doc(hidden)]
pub fn encode_attribute_data(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    out.push('"');
    for c in input.chars() {
        match c {
            '"' => out.push_str("&#x22;"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[doc(hidden)]
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
