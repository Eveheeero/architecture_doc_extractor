use super::Section;
use crate::intel::result::Instruction;
use regex::Regex;

pub(super) fn parse_now_section(instruction: &Instruction, line: impl AsRef<str>) -> Section {
    let footnote = Regex::new("(\\d)\\.\x01").unwrap();
    let line: &str = line.as_ref();
    match () {
        () if line.starts_with("Opcode\x01")
            || line.starts_with("Opcode/")
            || line.starts_with("Opcode /")
            || line == "Opcode" =>
        {
            Section::InstructionsStart
        }
        () if ((line.starts_with(&format!("{title}-", title = instruction.title))
            || line.starts_with(&format!("{title} -", title = instruction.title)))
            && !line.contains('.'))
            || line == "CMPS/CMPSB/CMPSW/CMPSD/CMP" =>
        {
            Section::TitleStart
        }
        () if line.trim() == "Description" => Section::Description,
        () if line.trim() == "Operation" => Section::Operation,
        () if line.trim() == "Flags Affected" => Section::FlagsAffected,
        () if line.trim_end().ends_with(" Exceptions") => {
            Section::Exceptions(line.trim().to_owned())
        }
        () if footnote.is_match(line) => {
            let num = footnote
                .captures(line)
                .unwrap()
                .get(1)
                .unwrap()
                .as_str()
                .parse::<u8>()
                .unwrap();
            Section::FootNote(num)
        }
        _ => Section::None,
    }
}
