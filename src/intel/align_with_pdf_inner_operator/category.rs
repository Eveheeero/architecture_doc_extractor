use crate::intel::category::Category;
use regex::Regex;
use std::sync::OnceLock;

pub(super) fn parse_category(data: impl AsRef<str>) -> Category {
    static SUMMARY: OnceLock<Regex> = OnceLock::new();
    static OPCODE_DESCRIPTION_START: OnceLock<Regex> = OnceLock::new();
    static OPCODE_DESCRIPTION_END: OnceLock<Regex> = OnceLock::new();
    static INSTRUCTION_OPERAND_ENCODING: OnceLock<Regex> = OnceLock::new();
    static DESCRIPTION: OnceLock<Regex> = OnceLock::new();
    static OPERATION: OnceLock<Regex> = OnceLock::new();
    static FLAGAFFECTED: OnceLock<Regex> = OnceLock::new();
    static EXCEPTIONS: OnceLock<Regex> = OnceLock::new();
    static IGNORE: OnceLock<Regex> = OnceLock::new();
    static INTRINSIC_EQUIVALENT_START: OnceLock<Regex> = OnceLock::new();
    let summary = SUMMARY.get_or_init(|| Regex::new("^[^a-z0-9 ][^a-z ]* ?-.+$").unwrap());
    // Opcode로 시작해서 여러 줄 거쳐서 Description으로 끝나는 경우
    let opcode_description_start =
        OPCODE_DESCRIPTION_START.get_or_init(|| Regex::new("^Opcode").unwrap());
    let opcode_description_end =
        OPCODE_DESCRIPTION_END.get_or_init(|| Regex::new("Description$").unwrap());
    let instruction_operand_encoding = INSTRUCTION_OPERAND_ENCODING
        .get_or_init(|| Regex::new("^Op/En.*Operand 1Operand 2Operand 3Operand 4$").unwrap());
    let description = DESCRIPTION.get_or_init(|| Regex::new("^Description$").unwrap());
    let operation = OPERATION.get_or_init(|| Regex::new("^Operation$").unwrap());
    let flag_effected = FLAGAFFECTED.get_or_init(|| Regex::new("^Flags Affected$").unwrap());
    let exceptions = EXCEPTIONS.get_or_init(|| {
        Regex::new(
            "^(.* Mode Exceptions|SIMD Floating-Point Exceptions|Other Exceptions|Exceptions)$",
        )
        .unwrap()
    });
    let ignore = IGNORE.get_or_init(|| Regex::new("(^Instruction Operand Encoding$)").unwrap());
    let intrinsic_equivalent = "Intel C/C++ Compiler Intrinsic Equivalent";
    let intrinsic_equivalent_start =
        INTRINSIC_EQUIVALENT_START.get_or_init(|| Regex::new("^Intel C/C$").unwrap());

    let data = data.as_ref();
    match () {
        () if summary.is_match(data) => Category::Summary,
        () if description.is_match(data) => Category::Description,
        () if opcode_description_start.is_match(data) && opcode_description_end.is_match(data) => {
            Category::OpcodeDescription
        }
        () if opcode_description_start.is_match(data) => Category::OpcodeDescriptionStart,
        () if instruction_operand_encoding.is_match(data) => Category::InstructionOperandEncoding,
        () if operation.is_match(data) => Category::Operation,
        () if flag_effected.is_match(data) => Category::FlagsAffected,
        () if exceptions.is_match(data) => Category::Exceptions,
        () if ignore.is_match(data) => Category::NeedIgnore,
        () if intrinsic_equivalent == data => Category::IntrinsicEquivalent,
        () if intrinsic_equivalent_start.is_match(data) => Category::IntrinsicEquivalentStart,
        _ => Category::None,
    }
}
