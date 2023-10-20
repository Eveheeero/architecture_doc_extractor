#![allow(unused_imports)]

mod page;
mod text;

pub(crate) use page::{get_page_contents, get_page_contents2};
pub(crate) use text::extract_tj;
