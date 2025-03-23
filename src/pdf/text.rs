use lopdf::{content::Operation, Document, Object};
use rayon::prelude::*;
use std::sync::Mutex;

/// pdf의 TJ operator에서 문자열을 추출한다.
/// TJ ex -> ["abc", 3(공백사이즈), "def"] -> "abc    def"
pub(crate) fn extract_tj<'obj>(
    doc: &'obj Document,
    obj: &'obj Object,
) -> Box<dyn Iterator<Item = u8> + 'obj> {
    match obj {
        Object::String(string, _) => Box::new(string.iter().copied()),
        Object::Integer(o) => {
            if o.abs() > 800 {
                Box::new(std::iter::once(1))
            } else {
                Box::new(std::iter::empty())
            }
        }
        Object::Real(o) => {
            if o.abs() > 800.0 {
                Box::new(std::iter::once(1))
            } else {
                Box::new(std::iter::empty())
            }
        }
        Object::Array(o) => Box::new(o.iter().map(|o| extract_tj(doc, o)).flatten()),
        _ => unreachable!(),
    }
}

fn extract_num(obj: &Object) -> f32 {
    match obj {
        Object::Integer(o) => *o as f32,
        Object::Real(o) => *o,
        _ => unimplemented!(),
    }
}

const PDF_TEXT_HEIGHT_FACTOR: f32 = 1.35; /* line factor, if error, change to 1.4 */

/// pdf 페이지 내부 정렬 순서에 따라 텍스트 파싱
pub(crate) fn operator_to_texts(
    doc: &Document,
    data: impl IntoIterator<Item = Operation>,
) -> Vec<String> {
    let mut last_position = (0.0, 0.0);
    let mut text_height = 0.0;
    let mut text_width = 0.0;
    let mut result: Vec<PdfInnerText> = data
        .into_iter()
        .filter(|op| {
            matches!(
                op.operator.as_str(),
                "Tj" | "TJ" | "TD" | "Td" | "Tm" | "Tlm" | "T*"
            )
        })
        .filter_map(|op| {
            if op.operator == "T*" {
                last_position.1 -= text_height * PDF_TEXT_HEIGHT_FACTOR;
                return None;
            } else if matches!(op.operator.as_str(), "Td" | "TD") {
                last_position.0 += extract_num(&op.operands[0]) * text_width;
                last_position.1 += extract_num(&op.operands[1]) * text_height;
                return None;
            } else if matches!(op.operator.as_str(), "Tm" | "Tlm") {
                if extract_num(&op.operands[0]) == extract_num(&op.operands[3])
                    && extract_num(&op.operands[1]) == 0.0
                    && extract_num(&op.operands[2]) == 0.0
                {
                    last_position.0 = extract_num(&op.operands[4]);
                    last_position.1 = extract_num(&op.operands[5]);
                }
                text_width = extract_num(&op.operands[0]);
                text_height = extract_num(&op.operands[3]);
                return None;
            }

            let line = op
                .operands
                .iter()
                .map(|operand| {
                    extract_tj(&doc, operand)
                        .map(|c| match c {
                            b'\n' => b' ',
                            c => c,
                        })
                        .map(|x| x as u16)
                        .collect::<Vec<u16>>()
                })
                .flatten()
                .collect::<Vec<u16>>();
            let line = String::from_utf16_lossy(&line);
            // 특수문자 제거
            let line = line
                .replace("\u{92}", "'")
                .replace("\\\u{93}", "\"")
                .replace("\\\u{94}", "\"")
                .replace("\\\u{95}", " - ")
                .replace("\u{93}", "\"")
                .replace("\u{94}", "\"")
                .replace("\u{95}", " - ")
                .replace("\u{96}", "-")
                .replace("\u{97}", "-")
                .replace("\u{8a}", "-");

            let mut text_position = last_position;
            text_position.1 += text_height;
            if false {
                println!("{} {:?}", op.operator, op.operands);
            }
            Some(PdfInnerText {
                text: line,
                start_position: text_position,
                text_width,
                text_height,
            })
        })
        .collect();
    if false {
        for i in &result {
            println!(
                "[{}:{}({})] {}",
                i.start_position.0, i.start_position.1, i.text_height, i.text
            );
        }
    }
    result.sort_by(|a, b| {
        let y = {
            if a.is_same_line(b) {
                std::cmp::Ordering::Equal
            } else if a.is_higher_than(b) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        };
        if y == std::cmp::Ordering::Equal {
            a.get_x_position().partial_cmp(&b.get_x_position()).unwrap()
        } else {
            y
        }
    });
    result
        .into_iter()
        .map(|x| x.get_text().to_owned())
        .collect()
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct PdfInnerText {
    text: String,
    /// (x, height)
    start_position: (f32, f32),
    text_width: f32,
    text_height: f32,
}
impl PdfInnerText {
    pub(crate) fn get_text(&self) -> &str {
        &self.text
    }
    pub(crate) fn is_higher_than(&self, other: &Self) -> bool {
        self.start_position.1 > other.start_position.1 && !self.is_same_line(other)
    }
    pub(crate) fn is_same_line(&self, other: &Self) -> bool {
        let (higher, lower) = if self.start_position.1 > other.start_position.1 {
            (self, other)
        } else {
            (other, self)
        };
        higher.start_position.1 - higher.text_height < lower.start_position.1
    }
    pub(crate) fn get_x_position(&self) -> f32 {
        self.start_position.0
    }
}
