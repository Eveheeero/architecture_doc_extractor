#![allow(dead_code)]

use std::io::Read;

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
#[derive(Debug)]
pub struct PdfFonts<'pdf>(&'pdf Document, &'pdf lopdf::Dictionary);
#[derive(Debug)]
pub enum PdfFont<'pdf> {
    Regular {
        first_char: usize,
        widths: Box<[f32]>,
        doc: &'pdf Document,
        font_descripter: &'pdf lopdf::Dictionary,
    },
    CidFont {
        doc: &'pdf Document,
        font: &'pdf lopdf::Dictionary,
        to_unicode: ToUnicode,
    },
}
#[derive(Debug)]
pub struct ToUnicode {
    origin: String,
    mapping: std::collections::HashMap<[u8; 2], [u8; 2]>,
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
        let font_descripter = get_font_descripter(self.0, font);
        PdfFont::Regular {
            first_char: first_char as usize,
            widths: widths
                .iter()
                .map(|w| w.as_i64().unwrap() as f32 / 1000.0)
                .collect(),
            doc: self.0,
            font_descripter,
        }
    }
    fn get_cidfont(&self, font: &'pdf lopdf::Dictionary) -> PdfFont<'pdf> {
        // {'/BaseFont': '/HKLMCJ+Cambria', '/DescendantFonts': [IndirectObject(16704, 0, 123145300088704)], '/Encoding': '/Identity-H', '/Subtype': '/Type0', '/ToUnicode': IndirectObject(7633, 0, 123145300088704), '/Type': '/Font'}
        // Descendant Font
        // {'/BaseFont': '/HKLMCJ+Cambria', '/CIDSystemInfo': {'/Ordering': 'Identity', '/Registry': 'Adobe', '/Supplement': 0}, '/CIDToGIDMap': '/Identity', '/DW': 1000, '/FontDescriptor': IndirectObject(16705, 0, 123145300088704), '/Subtype': '/CIDFontType2', '/Type': '/Font', '/W': [939, [554], 950, 951, 554, 955, [851]]}
        // ToUnicode (Decompressed by zlib)
        // /CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo <<\n/Registry (HKLMCJ+TT35+0) /Ordering (T42UV) /Supplement 0 >> def\n/CMapName /HKLMCJ+TT35+0 def\n/CMapType 2 def\n1 begincodespacerange <03ab> <03bb> endcodespacerange\n2 beginbfchar\n<03ab> <2212>\n<03bb> <221E>\nendbfchar\n1 beginbfrange\n<03b6> <03b7> <2264>\nendbfrange\nendcmap CMapName currentdict /CMap defineresource pop end end\n

        let to_unicode = self
            .0
            .dereference(font.get(b"ToUnicode").unwrap())
            .unwrap()
            .1
            .as_stream()
            .unwrap();
        let mut reader = flate2::bufread::ZlibDecoder::new(&*to_unicode.content);
        let mut to_unicode = String::new();
        reader.read_to_string(&mut to_unicode).unwrap();
        let to_unicode = parse_tounicode(to_unicode);
        PdfFont::CidFont {
            doc: self.0,
            font,
            to_unicode,
        }
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
            PdfFont::CidFont { .. } => todo!(),
        }
    }
    pub fn get_cid_width(&self, hex: [u8; 2]) -> f32 {
        let PdfFont::CidFont { doc, font, .. } = self else {
            unreachable!()
        };
        todo!()
    }
    pub fn get_cid_char(&self, hex: [u8; 2]) -> char {
        let PdfFont::CidFont { doc, font, .. } = self else {
            unreachable!()
        };
        todo!()
    }
    fn get_font_file(&self) -> &Vec<u8> {
        match self {
            PdfFont::Regular {
                doc,
                font_descripter,
                ..
            } => get_font_file(doc, font_descripter),
            PdfFont::CidFont { .. } => unreachable!(),
        }
    }
}

fn get_font_descripter<'pdf>(
    doc: &'pdf Document,
    font: &'pdf lopdf::Dictionary,
) -> &'pdf lopdf::Dictionary {
    doc.dereference(font.get(b"FontDescriptor").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap()
}

fn get_font_file<'pdf>(
    doc: &'pdf Document,
    font_descripter: &'pdf lopdf::Dictionary,
) -> &'pdf Vec<u8> {
    let font_file_key = font_descripter
        .as_hashmap()
        .keys()
        .find(|k| k.starts_with(b"FontFile"))
        .unwrap();
    &doc.dereference(font_descripter.get(&font_file_key).unwrap())
        .unwrap()
        .1
        .as_stream()
        .unwrap()
        .content
}

fn parse_tounicode(origin: String) -> ToUnicode {
    let mut mapping = std::collections::HashMap::new();
    use itertools::Itertools;

    let beginbfchar = origin.find("beginbfchar").unwrap();
    let endbfchar = origin.find("endbfchar").unwrap();
    let bfchar = &origin[beginbfchar..endbfchar];
    let beginbfrange = origin.find("beginbfrange").unwrap();
    let endbfrange = origin.find("endbfrange").unwrap();
    let bfrange = &origin[beginbfrange..endbfrange];

    let mut bfchar = bfchar.split_whitespace();
    bfchar.next();
    for mut bfchar in &bfchar.chunks(2) {
        let f = bfchar.next().unwrap();
        let f = f.trim_matches(|c| c == '<' || c == '>');
        let f = u16::from_str_radix(f, 16).unwrap();
        let f = f.to_be_bytes();
        let t = bfchar.next().unwrap();
        let t = t.trim_matches(|c| c == '<' || c == '>');
        let t = u16::from_str_radix(t, 16).unwrap();
        let t = t.to_be_bytes();
        mapping.insert(f, t);
    }

    todo!();

    ToUnicode { origin, mapping }
}

impl ToUnicode {
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn mapping_raw(&self) -> &std::collections::HashMap<[u8; 2], [u8; 2]> {
        &self.mapping
    }
    pub fn mapping(&self, hex: [u8; 2]) -> [u8; 2] {
        *self.mapping.get(&hex).unwrap()
    }
}
