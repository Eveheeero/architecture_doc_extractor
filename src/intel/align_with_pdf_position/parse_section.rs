use super::Section;
use crate::intel::result::Instruction;

pub(super) fn parse_now_section(instruction: &Instruction, line: impl AsRef<str>) -> Section {
    let line: &str = line.as_ref();
    match () {
        () if line.starts_with("Opcode\x01Instruction\x01Op") => Section::InstructionsStart,
        () if line.starts_with(&format!("{title}-", title = instruction.title))
            && !line.contains('.') =>
        {
            Section::TitleStart
        }
        () if line.trim() == "Description" => Section::Description,
        () if line.trim() == "Operation" => Section::Operation,
        () if line.trim() == "Flags Affected" => Section::FlagsAffected,
        () if line.trim_end().ends_with(" Exceptions") => {
            Section::Exceptions(line.trim().to_owned())
        }
        _ => Section::None,
    }
}
