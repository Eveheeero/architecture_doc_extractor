#![allow(dead_code)]

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

/// pdf 내부 operator순서에 따라 텍스트 파싱
pub(crate) fn operator_to_texts_align_with_pdf_inner_operator(
    doc: &Document,
    data: impl IntoParallelIterator<Item = Operation>,
) -> Vec<String> {
    data.into_par_iter()
        .filter(|op| op.operator.eq_ignore_ascii_case("tj"))
        .map(|op| {
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
            line
        })
        .collect()
}

fn extract_num(obj: &Object) -> f32 {
    match obj {
        Object::Integer(o) => *o as f32,
        Object::Real(o) => *o,
        _ => unimplemented!(),
    }
}

/// pdf 페이지 내부 정렬 순서에 따라 텍스트 파싱
pub(crate) fn operator_to_texts_align_with_pdf_position(
    doc: &Document,
    data: impl IntoIterator<Item = Operation>,
) -> Vec<String> {
    #[derive(Debug)]
    struct Text {
        text: String,
        /// (x, -y)
        start_position: (f32, f32),
        text_width: f32,
        text_height: f32,
    }
    let mut last_position = (0.0, 0.0);
    let mut text_height = 0.0;
    let mut text_width = 0.0;
    let mut result: Vec<Text> = data
        .into_iter()
        .filter(|op| {
            matches!(
                op.operator.as_str(),
                "Tj" | "TJ" | "TD" | "Td" | "Tm" | "Tlm" | "T*"
            )
        })
        .filter_map(|op| {
            if op.operator == "T*" {
                last_position.1 -= text_height * 1.35 /* line factor, if error, change to 1.4 */;
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
            Some(Text {
                text: line,
                start_position: text_position,
                text_width,
                text_height,
            })
        })
        .collect();
    result.sort_by(|a, b| {
        let y = a
            .start_position
            .1
            .partial_cmp(&b.start_position.1)
            .unwrap()
            .reverse();
        if y == std::cmp::Ordering::Equal {
            a.start_position.0.partial_cmp(&b.start_position.0).unwrap()
        } else {
            y
        }
    });
    result.into_iter().map(|x| x.text).collect()
}
