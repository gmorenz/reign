#![doc(html_logo_url = "https://reign.rs/images/media/reign.png")]
#![doc(html_root_url = "https://docs.rs/reign/0.2.1")]
#![doc = include_str!("../README.md")]

pub mod prelude;

#[cfg(feature = "view")]
pub use reign_view as view;
