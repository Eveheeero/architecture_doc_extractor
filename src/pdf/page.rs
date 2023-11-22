#![allow(dead_code)]

use lopdf::{content::Content, Document};
use rayon::prelude::*;

pub(crate) fn page_to_texts_align_with_pdf_inner_operator(
    doc: &Document,
    page: u32,
) -> Vec<String> {
    super::operator_to_texts_align_with_pdf_inner_operator(
        doc,
        get_page_contents(doc, page).operations,
    )
}

pub(crate) fn page_to_texts_align_with_pdf_position(doc: &Document, page: u32) -> Vec<String> {
    super::operator_to_texts_align_with_pdf_position(doc, get_page_contents(doc, page).operations)
}

pub(crate) fn get_page_contents(doc: &Document, page: u32) -> Content {
    let binding = doc.get_pages();
    let page = binding.get(&page).unwrap();
    let page = doc.get_object(*page).unwrap();
    let page_items = page.as_dict().unwrap();
    let page_items = doc
        .get_object(page_items.get(b"Contents").unwrap().as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap();
    Content::decode(&page_items.decompressed_content().unwrap()).unwrap()
}

pub(crate) fn get_page_contents2(doc: &Document, page: u32) -> Content {
    let pages = doc.get_pages();
    let page = pages.get(&page).unwrap();
    let page_contents = doc.get_page_contents(*page);
    let page_contents = doc
        .get_object(page_contents[0])
        .unwrap()
        .as_stream()
        .unwrap();
    Content::decode(&page_contents.decompressed_content().unwrap()).unwrap()
}
