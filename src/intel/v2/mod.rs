use crate::{intel::Instruction, pdf::v2::*};

pub(super) fn parse_instructions(d: Vec<(Vec<PdfString>, PdfBoxes)>) -> Vec<Instruction> {
    let mut result = Vec::new();
    let mut now=Instruction::default();
    todo!();
    result
}
