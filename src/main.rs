#[path = "intel/intel.rs"]
mod intel;
pub(crate) mod pdf;
#[cfg(test)]
mod tests;

use lopdf::Document;
use rayon::prelude::*;

fn main() {
    intel::main();
}

fn print_pages(doc: &Document, page: u32) -> Vec<String> {
    let page_items = pdf::get_page_contents(doc, page);
    let result: Vec<String> = page_items
        .operations
        .par_iter()
        .map(|op| {
            if !op.operator.eq_ignore_ascii_case("Tj") {
                return None;
            }
            let line = op
                .operands
                .iter()
                .map(|operand| {
                    pdf::extract_tj(&doc, operand)
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
            Some(line)
        })
        .filter(|x| x.is_some())
        .map(|x| x.unwrap())
        .collect();
    // ignore header, footer, page number
    result
}
