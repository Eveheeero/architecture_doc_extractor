#![allow(dead_code)]

use lopdf::{content::Content, Document};
use rayon::prelude::*;
use tracing::debug;

pub(crate) fn page_to_texts(doc: &Document, page: u32) -> Vec<String> {
    debug!("{}페이지 텍스트 추출중", page);
    super::operator_to_texts(doc, get_page_contents(doc, page).operations)
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

pub(crate) fn get_page_resources(doc: &Document, page: u32) -> &lopdf::Dictionary {
    let pages = doc.get_pages();
    let page = pages.get(&page).unwrap();
    let page = doc.get_object(*page).unwrap();
    let page = page.as_dict().unwrap();
    let resources = page.get(b"Resources").unwrap();
    doc.dereference(resources).unwrap().1.as_dict().unwrap()
}

pub(crate) fn get_char_width(
    doc: &Document,
    page: u32,
    font_name: impl AsRef<str>,
    c: char,
) -> f32 {
    let resources = get_page_resources(doc, page);
    let fonts = resources.get(b"Font").unwrap().as_dict().unwrap();
    let font = fonts.get(font_name.as_ref().as_bytes()).unwrap();
    let font = doc.dereference(font).unwrap().1.as_dict().unwrap();
    let widths = font.get(b"Widths").unwrap().as_array().unwrap();
    let first_char = font.get(b"FirstChar").unwrap().as_i64().unwrap();
    let index = c as i64 - first_char;
    widths[index as usize].as_i64().unwrap() as f32 / 1000.0
}

pub(crate) fn get_pdf_fonts(doc: &Document, page: u32) -> PdfFonts {
    let resources = get_page_resources(doc, page);
    let fonts = resources.get(b"Font").unwrap().as_dict().unwrap();
    PdfFonts(doc, fonts)
}
pub(crate) struct PdfFonts<'pdf>(&'pdf Document, &'pdf lopdf::Dictionary);
pub(crate) struct PdfFont {
    first_char: usize,
    widths: Box<[f32]>,
}
impl<'pdf> PdfFonts<'pdf> {
    pub fn get(&self, font_name: impl AsRef<str>) -> Option<PdfFont> {
        let Ok(font) = self.1.get(font_name.as_ref().as_bytes()) else {
            return None;
        };
        let font = self.0.dereference(font).unwrap().1.as_dict().unwrap();
        let widths = font.get(b"Widths").unwrap().as_array().unwrap();
        let first_char = font.get(b"FirstChar").unwrap().as_i64().unwrap();
        Some(PdfFont {
            first_char: first_char as usize,
            widths: widths
                .iter()
                .map(|w| w.as_i64().unwrap() as f32 / 1000.0)
                .collect(),
        })
    }
}
impl PdfFont {
    pub fn get_char_width(&self, c: char) -> f32 {
        let index = c as usize - self.first_char;
        self.widths[index]
    }
}
