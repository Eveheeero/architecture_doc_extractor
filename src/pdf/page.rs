#![allow(dead_code)]

use ab_glyph::Font;
use lopdf::{content::Content, Document};
use std::io::Read;
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
        font_arc: ab_glyph::FontArc,
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

        let descendant_font = &font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0];
        let descendant_font = self
            .0
            .dereference(descendant_font)
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        let font_descripter = get_font_descripter(self.0, descendant_font);
        let font_file = get_font_file(self.0, font_descripter);
        let font_arc = ab_glyph::FontArc::try_from_vec(font_file).unwrap();
        PdfFont::CidFont {
            doc: self.0,
            font,
            to_unicode,
            font_arc,
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
            PdfFont::CidFont { .. } => {
                // CidFont字 uses get_cid_width; fallback to default width
                tracing::warn!(byte = c, "get_char_width called on CidFont, returning default");
                0.5
            }
        }
    }
    pub fn get_cid_width(&self, hex: [u8; 2]) -> f32 {
        let PdfFont::CidFont {
            font_arc: font_ref, ..
        } = self
        else {
            unreachable!()
        };
        let c = self.get_cid_char(hex);
        let id = font_ref.glyph_id(c);
        font_ref.h_advance_unscaled(id) / 1000.0
    }
    pub fn get_cid_char(&self, hex: [u8; 2]) -> char {
        let PdfFont::CidFont { to_unicode, font_arc, .. } = self else {
            unreachable!()
        };
        match to_unicode.mapping(hex) {
            Some(c) => std::char::from_u32(u16::from_be_bytes(c) as u32).unwrap_or('\u{FFFD}'),
            None => {
                let glyph_id = u16::from_be_bytes(hex);
                let c = Self::cid_glyph_fallback(font_arc, glyph_id);
                if c == '\u{FFFD}' {
                    tracing::debug!(
                        glyph_id,
                        tounicode_origin = to_unicode.origin(),
                        "ToUnicode CMap for unmapped glyph"
                    );
                }
                c
            }
        }
    }
    /// Reverse-lookup: given a glyph ID, search Unicode codepoints to find which
    /// character maps to that glyph in the font's cmap table.
    /// Searches prioritized ranges first (math symbols, Latin, Greek, punctuation)
    /// then falls back to a broader BMP scan.
    fn cid_glyph_fallback(font_arc: &ab_glyph::FontArc, glyph_id: u16) -> char {
        use ab_glyph::Font;
        let target = ab_glyph::GlyphId(glyph_id);

        // Priority ranges: most likely characters in Intel SDM context
        let priority_ranges: &[std::ops::RangeInclusive<u32>] = &[
            0x2200..=0x22FF, // Mathematical Operators (≤, ≥, ≠, ∞, etc.)
            0x0020..=0x007E, // Basic ASCII
            0x00A0..=0x00FF, // Latin-1 Supplement (±, ×, ÷, etc.)
            0x0370..=0x03FF, // Greek and Coptic (α, β, etc.)
            0x2000..=0x206F, // General Punctuation
            0x2100..=0x214F, // Letterlike Symbols
            0x2150..=0x218F, // Number Forms
            0x0100..=0x024F, // Latin Extended-A/B
        ];
        for range in priority_ranges {
            for code in range.clone() {
                if let Some(c) = std::char::from_u32(code) {
                    if font_arc.glyph_id(c) == target {
                        return c;
                    }
                }
            }
        }

        // Broader BMP scan for remaining ranges not already covered
        for code in (0x0250..=0xFFFDu32)
            .filter(|c| !priority_ranges.iter().any(|r| r.contains(c)))
            .filter(|&c| !(0xD800..=0xDFFF).contains(&c))
        {
            if let Some(c) = std::char::from_u32(code) {
                if font_arc.glyph_id(c) == target {
                    return c;
                }
            }
        }

        tracing::warn!(glyph_id, "unmapped CID glyph, no reverse-lookup match");
        '\u{FFFD}'
    }
    fn get_font_file(&self) -> Vec<u8> {
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

fn get_font_file<'pdf>(doc: &'pdf Document, font_descripter: &'pdf lopdf::Dictionary) -> Vec<u8> {
    let font_file_key = font_descripter
        .as_hashmap()
        .keys()
        .find(|k| k.starts_with(b"FontFile"))
        .unwrap();
    let flated = &doc
        .dereference(font_descripter.get(&font_file_key).unwrap())
        .unwrap()
        .1
        .as_stream()
        .unwrap()
        .content;
    let mut reader = flate2::bufread::ZlibDecoder::new(&**flated);
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).unwrap();
    buf
}

/// Parse a single `<hex>` token from a character iterator, returning the u16 value.
fn parse_hex_token(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    extract: &dyn Fn(&str) -> u16,
) -> Option<u16> {
    // Skip whitespace
    while chars.peek().map_or(false, |c| c.is_whitespace()) {
        chars.next();
    }
    if chars.peek() != Some(&'<') {
        return None;
    }
    let mut token = String::new();
    for c in chars.by_ref() {
        token.push(c);
        if c == '>' {
            break;
        }
    }
    Some(extract(&token))
}

fn parse_tounicode(origin: String) -> ToUnicode {
    let mut mapping = std::collections::HashMap::new();
    use itertools::Itertools;

    let extract_hex_data = |s: &str| {
        let d = s.trim_matches(|c| c == '<' || c == '>');
        u16::from_str_radix(d, 16).unwrap()
    };

    // bfchar section (optional)
    if let (Some(begin), Some(end)) = (origin.find("beginbfchar"), origin.find("endbfchar")) {
        let bfchar = &origin[begin..end];
        let mut bfchar = bfchar.split_whitespace();
        bfchar.next();
        for mut bfchar in &bfchar.chunks(2) {
            let Some(f) = bfchar.next() else { break };
            let Some(t) = bfchar.next() else { break };
            let f = extract_hex_data(f).to_be_bytes();
            let t = extract_hex_data(t).to_be_bytes();
            mapping.insert(f, t);
        }
    }

    // bfrange section (optional)
    // Two forms per entry:
    //   Scalar: <srcStart> <srcEnd> <dstStart>  → sequential mapping
    //   Array:  <srcStart> <srcEnd> [<d1> <d2> ...]  → per-CID mapping
    if let (Some(begin), Some(end)) = (origin.find("beginbfrange"), origin.find("endbfrange")) {
        let section = &origin[begin..end];
        // Skip "beginbfrange"
        let body = &section["beginbfrange".len()..];

        let mut chars = body.chars().peekable();
        loop {
            // Skip whitespace
            while chars.peek().map_or(false, |c| c.is_whitespace()) {
                chars.next();
            }
            if chars.peek().is_none() {
                break;
            }

            // Parse <srcStart>
            let Some(src_start) = parse_hex_token(&mut chars, &extract_hex_data) else { break };
            // Parse <srcEnd>
            let Some(src_end) = parse_hex_token(&mut chars, &extract_hex_data) else { break };

            // Skip whitespace
            while chars.peek().map_or(false, |c| c.is_whitespace()) {
                chars.next();
            }

            if chars.peek() == Some(&'[') {
                // Array form: [<d1> <d2> ...]
                chars.next(); // skip '['
                for cid in src_start..=src_end {
                    if let Some(dst) = parse_hex_token(&mut chars, &extract_hex_data) {
                        mapping.insert(cid.to_be_bytes(), dst.to_be_bytes());
                    }
                }
                // Skip to closing ']'
                while chars.peek().map_or(false, |&c| c != ']') {
                    chars.next();
                }
                chars.next(); // skip ']'
            } else {
                // Scalar form: <dstStart>
                let Some(dst_start) = parse_hex_token(&mut chars, &extract_hex_data) else { break };
                for (i, cid) in (src_start..=src_end).enumerate() {
                    mapping.insert(cid.to_be_bytes(), (dst_start + i as u16).to_be_bytes());
                }
            }
        }
    }

    ToUnicode { origin, mapping }
}

impl ToUnicode {
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn mapping_raw(&self) -> &std::collections::HashMap<[u8; 2], [u8; 2]> {
        &self.mapping
    }
    pub fn mapping(&self, hex: [u8; 2]) -> Option<[u8; 2]> {
        self.mapping.get(&hex).copied()
    }
}
