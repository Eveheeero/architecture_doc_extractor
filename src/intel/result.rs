use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub(crate) struct MdTable {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
}

impl MdTable {
    pub(crate) fn to_md_lines(&self) -> Vec<String> {
        if self.headers.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        // Header row
        let header_line = format!("| {} |", self.headers.join(" | "));
        result.push(header_line);
        // Separator
        let sep_line = format!(
            "| {} |",
            self.headers
                .iter()
                .map(|_| "---".to_owned())
                .collect::<Vec<_>>()
                .join(" | ")
        );
        result.push(sep_line);
        // Data rows
        for row in &self.rows {
            // Pad row to header length
            let mut cells: Vec<String> = row.clone();
            cells.resize(self.headers.len(), String::new());
            result.push(format!("| {} |", cells.join(" | ")));
        }
        result
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Instruction {
    /// 메인 인스트럭션 (AAA)
    pub(crate) title: String,
    /// 인스트럭션 요약
    pub(crate) summary: String,
    /// 인스트럭션 변형별 설명 (인스트럭션 니모닉, 설명)
    pub(crate) instructions: Vec<(String, String)>,
    /// 상세설명
    pub(crate) description: Vec<String>,
    /// c 가상코드
    pub(crate) operation: String,
    /// 영향 받는 플래그
    pub(crate) flag_affected: String,
    /// 오류
    pub(crate) exceptions: HashMap<String, Vec<String>>,
    /// c/c++ 대체함수
    pub(crate) c_and_cpp_equivalent: Vec<String>,
    /// 기타 섹션 (섹션 이름, 본문 라인) - 등장 순서 유지
    pub(crate) other_sections: Vec<(String, Vec<String>)>,
    /// 테이블 (섹션 이름, 테이블)
    pub(crate) tables: Vec<(String, MdTable)>,
}

impl Instruction {
    /// Parse instruction name
    pub(crate) fn get_instructions_name(&self) -> Vec<String> {
        let data = self.title.clone();

        if data == "Jcc" {
            return [
                "Ja".into(),
                "Jae".into(),
                "Jb".into(),
                "Jbe".into(),
                "Jcxz".into(),
                "Jecxz".into(),
                "Jrcxz".into(),
                "Jz".into(),
                "Jg".into(),
                "Jge".into(),
                "Jl".into(),
                "Jle".into(),
                "Jnz".into(),
                "Jno".into(),
                "Jnp".into(),
                "Jns".into(),
                "Jo".into(),
                "Jp".into(),
                "Js".into(),
            ]
            .into();
        }

        if data.contains('[') {
            /* split with match([]) regex */
            let datas = Self::match_if_possible(data);
            let mut result = Vec::new();
            for data in datas {
                result.append(&mut Self::split_instruction_name_if_possible(data));
            }
            result
        } else {
            /* split with / and comma */
            Self::split_instruction_name_if_possible(data)
        }
    }
    fn match_if_possible(data: impl AsRef<str>) -> Vec<String> {
        let data = data.as_ref();
        let mut items = data.split('[');
        let before = items.next().unwrap().to_owned();
        let data = items.next().unwrap();
        let mut items = data.split(']');
        let middle = items.next().unwrap().to_owned();
        let selectable = middle
            .split(',')
            .map(|x| x.trim().to_owned())
            .collect::<Vec<String>>();
        let after = items.next().unwrap().to_owned();

        let mut datas = Vec::new();
        for select in selectable {
            let mut data = before.clone();
            data.push_str(&select);
            data.push_str(&after);
            datas.push(data);
        }

        let mut result = Vec::new();
        for data in datas.into_iter() {
            if data.contains('[') {
                result.append(&mut Self::match_if_possible(data));
            } else {
                result.push(data.to_owned());
            }
        }
        result
    }
    fn split_instruction_name_if_possible(data: impl AsRef<str>) -> Vec<String> {
        let data = data.as_ref();
        if data.contains("/") {
            let items: Vec<String> = data.split("/").map(|x| x.trim().to_owned()).collect();
            let mut result: Vec<String> = Vec::new();
            let etc_len_is_2 = items.len() >= 2 && items.iter().all(|x| x.len() == 2);
            for item in items.into_iter() {
                if item.len() <= 2 {
                    let mut first = result[0].clone();
                    first.pop();
                    if etc_len_is_2 {
                        first.pop();
                    }
                    first.push_str(&item);
                    result.append(&mut Self::split_comma(first));
                } else {
                    result.append(&mut Self::split_comma(item));
                }
            }
            result
        } else {
            [data.to_owned()].into()
        }
    }
    fn split_comma(data: impl AsRef<str>) -> Vec<String> {
        let data = data.as_ref();
        if !data.contains(',') {
            return [data.to_owned()].into();
        }
        let mut result = Vec::new();
        let mut items = data.split(',');
        let first = items.next().unwrap().to_owned();
        let rest = items.collect::<Vec<&str>>().join(",");
        result.push(first);
        result.append(&mut Self::split_comma(rest));
        result
    }
    /// Instruction to result string
    pub(crate) fn into_md(self) -> Vec<String> {
        let mut result = Vec::new();

        // 제목
        result.push(format!("# {title}", title = self.title));

        // 요약
        result.push("".to_owned());
        result.push(format!("{summary}", summary = self.summary));

        // 인스트럭션 변형별 설명
        if !self.instructions.is_empty() {
            result.push("".to_owned());
            for (mnemonic, desc) in &self.instructions {
                result.push(format!("- `{mnemonic}` — {desc}"));
            }
        }

        // 테이블 (Opcode, Instruction Operand Encoding 등)
        for (section_name, table) in &self.tables {
            result.push("".to_owned());
            result.push(format!("## {section_name}"));
            result.push("".to_owned());
            result.append(&mut table.to_md_lines());
        }

        // 설명
        let mut description = self.get_description();
        if !description.is_empty() {
            result.push("".to_owned());
            result.push("## Description".to_owned());
            result.push("".to_owned());
            result.append(&mut description);
        }
        drop(description);

        // C/C++ Compiler Intrinsic Equivalent
        if !self.c_and_cpp_equivalent.is_empty() {
            result.push("".to_owned());
            result.push("## Intel C/C++ Compiler Intrinsic Equivalent".to_owned());
            result.push("".to_owned());
            for line in &self.c_and_cpp_equivalent {
                result.push(line.clone());
            }
        }

        // Flags affected
        if !self.flag_affected.is_empty() {
            result.push("".to_owned());
            result.push("## Flags Affected".to_owned());
            result.push("".to_owned());
            result.push(self.flag_affected.clone());
        }

        // 기타 섹션
        for (name, lines) in &self.other_sections {
            if !lines.is_empty() {
                result.push("".to_owned());
                result.push(format!("## {name}"));
                result.push("".to_owned());
                for line in lines {
                    result.push(line.clone());
                }
            }
        }

        // 예외
        if !self.exceptions.is_empty() {
            result.push("".to_owned());
            result.push("## Exceptions".to_owned());
            result.push("".to_owned());
            for (kind, exceptions) in &self.exceptions {
                result.push(format!("### {kind}"));
                result.push("".to_owned());
                for exception in exceptions {
                    result.push(format!("- {exception}"));
                }
                result.push("".to_owned());
            }
        }

        // Operation (C pseudocode)
        if !self.operation.is_empty() {
            result.push("".to_owned());
            result.push("## Operation".to_owned());
            result.push("".to_owned());
            result.push("```C".to_owned());
            result.append(&mut self.get_reformed_operation());
            result.push("```".to_owned());
        }

        result.push("".to_owned());
        result
    }

    fn get_description(&self) -> Vec<String> {
        if self.description.is_empty() {
            return Vec::new();
        }
        let long = self.description.join(" ");
        long.split(". ")
            .map(|s| {
                let s = s.trim();
                if s.ends_with('.') {
                    s.to_owned()
                } else {
                    format!("{s}.")
                }
            })
            .collect()
    }
    fn get_reformed_operation(&self) -> Vec<String> {
        self.operation
            .lines()
            .map(|line| line.to_owned())
            .collect()
    }
}
