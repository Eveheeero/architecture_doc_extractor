use super::result::{ArmEncoding, ArmInstruction, BitField};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::HashMap;
use tracing::{debug, warn};

/// 모든 XML 파일을 파싱하여 ArmInstruction 벡터 반환
pub(crate) fn parse_all_instructions(xml_files: HashMap<String, Vec<u8>>) -> Vec<ArmInstruction> {
    let mut instructions = Vec::new();
    let mut filenames: Vec<_> = xml_files.keys().cloned().collect();
    filenames.sort();

    for filename in filenames {
        let data = &xml_files[&filename];
        match parse_instruction_xml(data) {
            Some(instr) => {
                if !instr.heading.is_empty() {
                    instructions.push(instr);
                }
            }
            None => {
                // 에일리어스이거나 파싱 불가 — 스킵
            }
        }
    }

    debug!("Parsed {} ARM instructions", instructions.len());
    instructions
}

/// 단일 XML 파일 파싱. 에일리어스면 None 반환.
fn parse_instruction_xml(data: &[u8]) -> Option<ArmInstruction> {
    let mut reader = Reader::from_reader(data);
    // trim_text를 끄면 공백이 보존됨
    reader.config_mut().trim_text(false);

    let mut instr = ArmInstruction::default();
    let mut buf = Vec::new();

    // 상태 추적
    let mut in_instructionsection = false;
    let mut in_heading = false;
    let mut in_desc = false;
    let mut in_brief = false;
    let mut in_authored = false;
    let mut in_operationalnotes = false;
    let mut in_classes = false;
    let mut in_iclass = false;
    let mut in_regdiagram = false;
    let mut in_encoding = false;
    let mut in_asmtemplate = false;
    let mut in_ps_section = false;
    let mut in_pstext = false;
    let mut in_explanations = false;
    let mut in_explanation = false;
    let mut in_symbol = false;
    let mut in_account_intro = false;
    let mut in_definition_intro = false;
    let mut in_def_table = false;
    let mut in_def_table_row = false;
    let mut in_def_table_entry = false;
    let mut in_def_table_thead = false;
    let mut in_alias_list = false;
    let mut in_aliasref = false;
    let mut in_aliasref_text = false;
    let mut in_aliaspref = false;
    let mut in_para = false;
    let mut in_list = false;
    let mut in_listitem = false;
    let mut in_content = false;

    let mut current_encoding = ArmEncoding::default();
    let mut current_box = BitField::default();
    let mut bitfields_captured = false;
    let mut pstext_section = String::new();
    let mut current_symbol = String::new();
    let mut current_explanation_text = String::new();
    let mut asm_template_parts: Vec<String> = Vec::new();
    let mut para_text = String::new();
    let mut content_text = String::new();
    // 중첩 ps_section 깊이 (classes 내부 vs 최상위)
    let mut ps_section_depth = 0;
    // iclass별 decode pseudocode 분리
    let mut current_iclass_name = String::new();
    let mut decode_sections: Vec<(String, String)> = Vec::new();
    let mut current_decode = String::new();
    // definition table 파싱
    let mut def_table_rows: Vec<Vec<String>> = Vec::new();
    let mut def_table_current_row: Vec<String> = Vec::new();
    let mut def_table_entry_text = String::new();
    // operational notes 중복 방지
    let mut operational_notes_set: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    // heading 텍스트
    let mut heading_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"instructionsection" => {
                        // type="alias" 이면 스킵
                        let type_val = get_attr(e, "type").unwrap_or_default();
                        if type_val == "alias" {
                            return None;
                        }
                        instr.id = get_attr(e, "id").unwrap_or_default();
                        in_instructionsection = true;
                    }
                    b"heading" if in_instructionsection => {
                        in_heading = true;
                        heading_text.clear();
                    }
                    b"docvar" if in_instructionsection && !in_classes => {
                        let key = get_attr(e, "key").unwrap_or_default();
                        let val = get_attr(e, "value").unwrap_or_default();
                        match key.as_str() {
                            "mnemonic" => instr.mnemonic = val,
                            "instr-class" => instr.instr_class = val,
                            _ => {}
                        }
                    }
                    b"desc" if in_instructionsection => in_desc = true,
                    b"brief" if in_desc => in_brief = true,
                    b"authored" if in_desc => in_authored = true,
                    b"para" => {
                        in_para = true;
                        para_text.clear();
                    }
                    b"operationalnotes" if in_instructionsection => in_operationalnotes = true,
                    b"list" if in_operationalnotes => in_list = true,
                    b"listitem" if in_list => in_listitem = true,
                    b"content" if in_listitem => {
                        in_content = true;
                        content_text.clear();
                    }
                    b"alias_list" if in_instructionsection => in_alias_list = true,
                    b"aliasref" if in_alias_list => {
                        in_aliasref = true;
                    }
                    b"text" if in_aliasref => {
                        in_aliasref_text = true;
                    }
                    b"aliaspref" if in_aliasref => {
                        in_aliaspref = true;
                    }
                    b"classes" if in_instructionsection => in_classes = true,
                    b"iclass" if in_classes => {
                        in_iclass = true;
                        current_iclass_name = get_attr(e, "name").unwrap_or_default();
                        current_decode.clear();
                    }
                    b"regdiagram" if in_iclass && !bitfields_captured => in_regdiagram = true,
                    b"box" if in_regdiagram => {
                        current_box = BitField::default();
                        current_box.hibit = get_attr(e, "hibit")
                            .unwrap_or_default()
                            .parse()
                            .unwrap_or(0);
                        current_box.width = get_attr(e, "width")
                            .unwrap_or_else(|| "1".to_owned())
                            .parse()
                            .unwrap_or(1);
                        current_box.name = get_attr(e, "name").unwrap_or_default();
                    }
                    b"c" if in_regdiagram => {}
                    b"encoding" if in_iclass => {
                        in_encoding = true;
                        current_encoding = ArmEncoding::default();
                        current_encoding.name = get_attr(e, "name").unwrap_or_default();
                        current_encoding.label = get_attr(e, "label").unwrap_or_default();
                    }
                    b"asmtemplate" if in_encoding => {
                        in_asmtemplate = true;
                        asm_template_parts.clear();
                    }
                    b"a" if in_asmtemplate => {
                        // <a> 태그 내부 텍스트도 수집 — 공백 보존됨
                    }
                    b"ps_section" if in_instructionsection => {
                        ps_section_depth += 1;
                        in_ps_section = true;
                    }
                    b"pstext" if in_ps_section => {
                        in_pstext = true;
                        pstext_section = get_attr(e, "section").unwrap_or_default();
                    }
                    b"a" if in_pstext => {
                        // 의사코드 내 링크 — 텍스트만 수집
                    }
                    b"explanations" if in_instructionsection => in_explanations = true,
                    b"explanation" if in_explanations => {
                        in_explanation = true;
                        current_symbol.clear();
                        current_explanation_text.clear();
                    }
                    b"symbol" if in_explanation => {
                        in_symbol = true;
                    }
                    b"intro" if in_explanation => {
                        if in_explanations {
                            in_account_intro = true;
                        }
                    }
                    b"definition" if in_explanation => {
                        in_definition_intro = true;
                    }
                    b"table" if in_definition_intro => {
                        in_def_table = true;
                        def_table_rows.clear();
                    }
                    b"thead" if in_def_table => {
                        in_def_table_thead = true;
                    }
                    b"row" if in_def_table => {
                        in_def_table_row = true;
                        def_table_current_row.clear();
                    }
                    b"entry" if in_def_table_row => {
                        in_def_table_entry = true;
                        def_table_entry_text.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"instructionsection" => in_instructionsection = false,
                    b"heading" => {
                        in_heading = false;
                        let trimmed = heading_text.trim().to_owned();
                        if !trimmed.is_empty() {
                            instr.heading = trimmed;
                        }
                    }
                    b"desc" => in_desc = false,
                    b"brief" => in_brief = false,
                    b"authored" => in_authored = false,
                    b"para" => {
                        in_para = false;
                        let text = para_text.trim().to_owned();
                        if !text.is_empty() {
                            if in_brief {
                                instr.brief = text;
                            } else if in_authored {
                                instr.description.push(text);
                            } else if in_operationalnotes && !in_list {
                                if operational_notes_set.insert(text.clone()) {
                                    instr.operational_notes.push(text);
                                }
                            } else if in_account_intro || in_definition_intro {
                                if !current_explanation_text.is_empty() {
                                    current_explanation_text.push(' ');
                                }
                                current_explanation_text.push_str(&text);
                            }
                        }
                        para_text.clear();
                    }
                    b"operationalnotes" => {
                        in_operationalnotes = false;
                        in_list = false;
                    }
                    b"list" => in_list = false,
                    b"listitem" => in_listitem = false,
                    b"content" => {
                        if in_content {
                            let text = content_text.trim().to_owned();
                            if !text.is_empty() && in_operationalnotes {
                                if operational_notes_set.insert(text.clone()) {
                                    instr.operational_notes.push(text);
                                }
                            }
                            in_content = false;
                        }
                    }
                    b"alias_list" => in_alias_list = false,
                    b"aliasref" => in_aliasref = false,
                    b"text" if in_aliasref => in_aliasref_text = false,
                    b"aliaspref" => in_aliaspref = false,
                    b"classes" => in_classes = false,
                    b"iclass" => {
                        // iclass 끝: decode pseudocode 저장
                        let trimmed = current_decode.trim().to_owned();
                        if !trimmed.is_empty() {
                            decode_sections.push((current_iclass_name.clone(), trimmed));
                        }
                        in_iclass = false;
                    }
                    b"regdiagram" => {
                        in_regdiagram = false;
                        if !instr.bitfields.is_empty() {
                            bitfields_captured = true;
                        }
                    }
                    b"box" if in_regdiagram => {
                        instr.bitfields.push(current_box.clone());
                    }
                    b"encoding" if in_iclass => {
                        in_encoding = false;
                        if !current_encoding.asm_template.is_empty() {
                            instr.encodings.push(current_encoding.clone());
                        }
                    }
                    b"asmtemplate" => {
                        in_asmtemplate = false;
                        current_encoding.asm_template =
                            asm_template_parts.join("").trim().to_owned();
                    }
                    b"ps_section" => {
                        ps_section_depth -= 1;
                        if ps_section_depth == 0 {
                            in_ps_section = false;
                        }
                    }
                    b"pstext" => {
                        in_pstext = false;
                    }
                    b"explanations" => in_explanations = false,
                    b"explanation" => {
                        if !current_symbol.is_empty() && !current_explanation_text.is_empty() {
                            // 공백 정규화: 모든 연속 공백/개행을 단일 공백으로
                            let normalized = normalize_whitespace(&current_explanation_text);
                            instr
                                .operand_explanations
                                .push((current_symbol.trim().to_owned(), normalized));
                        }
                        in_explanation = false;
                        in_account_intro = false;
                        in_definition_intro = false;
                    }
                    b"symbol" => in_symbol = false,
                    b"intro" if in_explanation => {
                        in_account_intro = false;
                    }
                    b"definition" => {
                        in_definition_intro = false;
                    }
                    b"table" if in_def_table => {
                        // definition table 완료 — 텍스트로 변환 (단일행, 개행 없이)
                        if !def_table_rows.is_empty() {
                            let table_text = format_def_table(&def_table_rows);
                            if !current_explanation_text.is_empty() {
                                current_explanation_text.push(' ');
                            }
                            current_explanation_text.push_str(&table_text);
                        }
                        in_def_table = false;
                    }
                    b"thead" => in_def_table_thead = false,
                    b"row" if in_def_table => {
                        if !def_table_current_row.is_empty() && !in_def_table_thead {
                            def_table_rows.push(def_table_current_row.clone());
                        }
                        in_def_table_row = false;
                    }
                    b"entry" if in_def_table_entry => {
                        def_table_current_row.push(def_table_entry_text.trim().to_owned());
                        in_def_table_entry = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if text.is_empty() {
                    buf.clear();
                    continue;
                }

                if in_heading {
                    heading_text.push_str(&text);
                }

                if in_para {
                    para_text.push_str(&text);
                } else if in_content && in_operationalnotes {
                    content_text.push_str(&text);
                }

                if in_def_table_entry {
                    def_table_entry_text.push_str(&text);
                }

                if in_asmtemplate {
                    asm_template_parts.push(text.clone());
                }

                if in_pstext {
                    match pstext_section.as_str() {
                        "Decode" | "decode" => {
                            if in_classes {
                                current_decode.push_str(&text);
                            } else {
                                instr.decode_pseudocode.push_str(&text);
                            }
                        }
                        "Execute" | "execute" => {
                            instr.operation.push_str(&text);
                        }
                        _ => {
                            // Postdecode 등
                            if !in_classes {
                                instr.operation.push_str(&text);
                            } else {
                                current_decode.push_str(&text);
                            }
                        }
                    }
                }

                if in_symbol {
                    current_symbol.push_str(&text);
                }

                if (in_account_intro || in_definition_intro) && !in_def_table {
                    if in_para {
                        // para에서 이미 처리됨
                    } else {
                        current_explanation_text.push_str(&text);
                    }
                }

                // alias: <text> 내부만 수집, <aliaspref> 내용은 무시
                if in_aliasref_text && !in_aliaspref {
                    let t = text.trim();
                    if !t.is_empty() {
                        instr.aliases.push(t.to_owned());
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                match e.name().as_ref() {
                    b"docvar" if in_instructionsection && !in_classes => {
                        let key = get_attr(e, "key").unwrap_or_default();
                        let val = get_attr(e, "value").unwrap_or_default();
                        match key.as_str() {
                            "mnemonic" if instr.mnemonic.is_empty() => instr.mnemonic = val,
                            "instr-class" if instr.instr_class.is_empty() => {
                                instr.instr_class = val
                            }
                            _ => {}
                        }
                    }
                    b"c" if in_regdiagram => {
                        // 비트 상수
                    }
                    _ => {}
                }
            }
            Err(e) => {
                warn!("XML 파싱 오류: {}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    // heading이 비어있으면 스킵
    if instr.heading.is_empty() {
        return None;
    }

    // mnemonic이 비어있으면 heading에서 첫 번째 단어 추출
    if instr.mnemonic.is_empty() {
        let first = instr
            .heading
            .split(|c: char| c == ' ' || c == ',' || c == '/')
            .next()
            .unwrap_or("")
            .trim();
        if !first.is_empty() {
            instr.mnemonic = first.to_owned();
        }
    }

    // decode pseudocode 조합: 여러 iclass에서 온 경우 분리
    if !decode_sections.is_empty() {
        let combined = if decode_sections.len() == 1 {
            decode_sections[0].1.clone()
        } else {
            decode_sections
                .iter()
                .map(|(name, code)| {
                    if name.is_empty() {
                        code.clone()
                    } else {
                        format!("// {name}\n{code}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        if instr.decode_pseudocode.is_empty() {
            instr.decode_pseudocode = combined;
        } else {
            instr.decode_pseudocode = format!("{}\n\n{combined}", instr.decode_pseudocode);
        }
    }

    Some(instr)
}

/// 연속 공백/개행을 단일 공백으로 정규화
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// definition 내부 table을 텍스트로 변환
fn format_def_table(rows: &[Vec<String>]) -> String {
    let mut result = Vec::new();
    for row in rows {
        if row.len() >= 2 {
            result.push(format!("{} → {}", row[0], row[1]));
        } else if row.len() == 1 {
            result.push(row[0].clone());
        }
    }
    result.join(", ")
}

/// XML 요소에서 속성 값 추출
fn get_attr(e: &BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes() {
        if let Ok(attr) = attr {
            if attr.key.as_ref() == name.as_bytes() {
                return Some(attr.unescape_value().unwrap_or_default().to_string());
            }
        }
    }
    None
}
