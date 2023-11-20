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

pub(crate) fn operator_to_texts(
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

pub(crate) fn operator_to_texts2(
    doc: &Document,
    data: impl IntoIterator<Item = Operation>,
) -> Vec<String> {
    struct Text {
        text: String,
        start_position: (f32, f32),
    }
    let mut last_position = (0.0, 0.0);
    let mut result: Vec<Text> = data
        .into_iter()
        .filter(|op| op.operator == "Tj" || op.operator == "TD" || op.operator == "TJ")
        .filter_map(|op| {
            if op.operator == "TD" {
                last_position = (extract_num(&op.operands[0]), extract_num(&op.operands[1]));
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

            Some(Text {
                text: line,
                start_position: last_position,
            })
        })
        .collect();
    result.sort_by(|a, b| {
        let y = a.start_position.1.partial_cmp(&b.start_position.1).unwrap();
        if y == std::cmp::Ordering::Equal {
            a.start_position.0.partial_cmp(&b.start_position.0).unwrap()
        } else {
            y
        }
    });
    result.into_iter().map(|x| x.text).collect()
}
