#![allow(dead_code)]

use lopdf::{content::Content, Document};
use tracing::debug;

pub fn page_to_texts_v1(doc: &Document, page: u32) -> Vec<String> {
    debug!("{}페이지 텍스트 추출중", page);
    crate::pdf::v1::operator_to_texts(doc, get_page_contents(doc, page).operations)
}

pub fn page_to_texts_v2(doc: &Document, page: u32) -> Vec<crate::pdf::v2::PdfString> {
    debug!("{}페이지 텍스트 추출중", page);
    let chars = crate::pdf::v2::operator_to_chars(
        crate::pdf::get_pdf_fonts(doc, page),
        get_page_contents(doc, page).operations,
    );
    let mut strings = crate::pdf::v2::detect_strings(chars);
    crate::pdf::v2::sort_strings(&mut strings);
    strings
}
pub fn page_to_boxes_v2(doc: &Document, page: u32) -> crate::pdf::v2::PdfBoxes {
    debug!("{}페이지 라인 추출중", page);
    crate::pdf::v2::operator_to_boxes(get_page_contents(doc, page).operations)
}

pub fn get_page_contents(doc: &Document, page: u32) -> Content {
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

pub fn get_page_contents2(doc: &Document, page: u32) -> Content {
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

pub fn get_page_resources(doc: &Document, page: u32) -> &lopdf::Dictionary {
    let pages = doc.get_pages();
    let page = pages.get(&page).unwrap();
    let page = doc.get_object(*page).unwrap();
    let page = page.as_dict().unwrap();
    let resources = page.get(b"Resources").unwrap();
    doc.dereference(resources).unwrap().1.as_dict().unwrap()
}

pub fn get_char_width(doc: &Document, page: u32, font_name: impl AsRef<str>, c: u8) -> f32 {
    let resources = get_page_resources(doc, page);
    let fonts = resources.get(b"Font").unwrap().as_dict().unwrap();
    let font = fonts.get(font_name.as_ref().as_bytes()).unwrap();
    let font = doc.dereference(font).unwrap().1.as_dict().unwrap();
    let widths = font.get(b"Widths").unwrap().as_array().unwrap();
    let first_char = font.get(b"FirstChar").unwrap().as_i64().unwrap();
    let index = c as i64 - first_char;
    widths
        .get(index as usize)
        .unwrap_or(&lopdf::Object::Integer(0))
        .as_i64()
        .unwrap() as f32
        / 1000.0
}

pub fn get_pdf_fonts(doc: &Document, page: u32) -> PdfFonts {
    let resources = get_page_resources(doc, page);
    let fonts = resources.get(b"Font").unwrap().as_dict().unwrap();
    PdfFonts(doc, fonts)
}
pub struct PdfFonts<'pdf>(&'pdf Document, &'pdf lopdf::Dictionary);
pub enum PdfFont<'pdf> {
    Regular {
        first_char: usize,
        widths: Box<[f32]>,
        doc: &'pdf Document,
        font_file: &'pdf lopdf::Dictionary,
    },
    CidFont {
        doc: &'pdf Document,
        font: &'pdf lopdf::Dictionary,
    },
}

impl<'pdf> PdfFonts<'pdf> {
    pub fn get(&self, font_name: impl AsRef<str>) -> PdfFont {
        let font = self.1.get(font_name.as_ref().as_bytes()).unwrap();
        let font = self.0.dereference(font).unwrap().1.as_dict().unwrap();
        // TODO Custom Encoding not covered
        if font.get(b"Subtype").unwrap().as_name_str().unwrap() != "Type0" {
            self.get_regular(&font)
        } else {
            self.get_cidfont(&font)
        }
    }
    fn get_regular(&'pdf self, font: &'pdf lopdf::Dictionary) -> PdfFont<'pdf> {
        let widths = font
            .get(b"Widths")
            .and_then(lopdf::Object::as_array)
            .unwrap();
        let first_char = font
            .get(b"FirstChar")
            .and_then(lopdf::Object::as_i64)
            .unwrap();
        let font_file = self
            .0
            .dereference(font.get(b"FontDescriptor").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        PdfFont::Regular {
            first_char: first_char as usize,
            widths: widths
                .iter()
                .map(|w| w.as_i64().unwrap() as f32 / 1000.0)
                .collect(),
            doc: self.0,
            font_file: font_file,
        }
    }
    fn get_cidfont(&self, font: &'pdf lopdf::Dictionary) -> PdfFont<'pdf> {
        PdfFont::CidFont { doc: self.0, font }
    }
}
impl<'pdf> PdfFont<'pdf> {
    pub fn get_char_width(&self, c: u8) -> f32 {
        match self {
            Self::Regular {
                first_char, widths, ..
            } => {
                let index = c as usize - first_char;
                widths[index]
            }
            PdfFont::CidFont { doc, font } => todo!(),
        }
    }
    pub fn get_cid_width(&self, hex: [u8; 2]) -> f32 {
        let PdfFont::CidFont { doc, font } = self else {
            unreachable!()
        };
        // get with ToUnicode (stream) and DescendantFonts (list of reference)
        /*
        >>> font.get("/TT35").get_object()
        {'/BaseFont': '/HKLMCJ+Cambria', '/DescendantFonts': [IndirectObject(16704, 0, 123145300088704)], '/Encoding': '/Identity-H', '/Subtype': '/Type0', '/ToUnicode': IndirectObject(7633, 0, 123145300088704), '/Type': '/Font'}
        >>> font.get("/TT35").get_object().get("/DescendantFonts")[0].get_object()
        {'/BaseFont': '/HKLMCJ+Cambria', '/CIDSystemInfo': {'/Ordering': 'Identity', '/Registry': 'Adobe', '/Supplement': 0}, '/CIDToGIDMap': '/Identity', '/DW': 1000, '/FontDescriptor': IndirectObject(16705, 0, 123145300088704), '/Subtype': '/CIDFontType2', '/Type': '/Font', '/W': [939, [554], 950, 951, 554, 955, [851]]}
         */
        todo!()
    }
    pub fn get_cid_char(&self, hex: [u8; 2]) -> char {
        let PdfFont::CidFont { doc, font } = self else {
            unreachable!()
        };
        todo!()
    }
    pub fn get_font_file(&self) -> &Vec<u8> {
        match self {
            PdfFont::Regular { doc, font_file, .. } => {
                let font_file_key = font_file
                    .as_hashmap()
                    .keys()
                    .find(|k| k.starts_with(b"FontFile"))
                    .unwrap();
                &doc.dereference(font_file.get(&font_file_key).unwrap())
                    .unwrap()
                    .1
                    .as_stream()
                    .unwrap()
                    .content
            }
            PdfFont::CidFont { .. } => unreachable!(),
        }
    }
}
