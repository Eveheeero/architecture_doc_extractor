use crate::{intel::Instruction, pdf::v2::*};
use std::collections::BTreeSet;

pub(super) fn parse_instructions(mut d: Vec<(Vec<PdfString>, PdfBoxes)>) -> Vec<Instruction> {
    let mut result = Vec::new();
    let mut now = Instruction::default();
    for (sorted_strings, boxes) in &mut d {
        /* 셀 준비 */
        boxes.prepare_cells();

        /* 스킵 설정 */
        let mut skip_index: BTreeSet<usize> = BTreeSet::new();
        // 헤더 푸터 제외
        skip_index.extend(get_header_index(sorted_strings));
        skip_index.extend(get_footer_index(sorted_strings));

        let (instruction, summary) = get_instruction_and_summary(sorted_strings);

        /* 페이지 내부 텍스트에 대해 반복 */
        for (string_index, _string) in sorted_strings.iter().enumerate() {
            if skip_index.contains(&string_index) {
                skip_index.remove(&string_index);
                continue;
            }
            let rect = _string.rect();
            let string = _string.get();
            let wrapping_cell = boxes.get_wrapping_cell(&rect);

            /* 셀 내부에 있으면 옆이나 아래 있는 셀들과 함께 분석 */

            /* string 분석 후 섹션 파악 후 정보 설정 */

            println!("{string}");
        }
    }
    todo!();
    result
}

fn get_header_index(_sorted_strings: &Vec<PdfString>) -> Vec<usize> {
    [0].into()
}
fn get_footer_index(sorted_strings: &Vec<PdfString>) -> Vec<usize> {
    let len = sorted_strings.len();
    [len - 2, len - 1].into()
}
/// (instruction, summary)
fn get_instruction_and_summary(sorted_strings: &Vec<PdfString>) -> (String, String) {
    const RE: std::cell::LazyCell<regex::Regex> =
        std::cell::LazyCell::new(|| regex::Regex::new(r"(^Vol\.)|(^\d-\d+Vol.)").unwrap());
    let len = sorted_strings.len();
    let (last_1, last_2) = (len - 2, len - 1);
    let instruction_line = if RE.is_match(&sorted_strings[last_1].get()) {
        sorted_strings[last_2].get()
    } else {
        sorted_strings[last_1].get()
    };
    // ANDNPDBitwise Logical AND NOT of Packed Double Precision Floating-Point Values
    todo!()
}
