use super::result::Instruction;

#[allow(dead_code)]
pub(super) fn parse_instructions(data: Vec<Vec<String>>) -> Vec<Instruction> {
    for page in data.into_iter() {
        for line in page.into_iter() {}
    }
    todo!()
}
