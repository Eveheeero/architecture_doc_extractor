use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub(super) struct Instruction {
    /// 메인 인스트럭션 (AAA)
    pub(super) title: String,
    /// 인스트럭션 요약
    pub(super) summary: String,
    /// 상세 인스트럭션 별 설명
    pub(super) instruction: (String, String),
    /// 상세설명
    pub(super) description: Vec<String>,
    /// c 가상코드
    pub(super) operation: String,
    /// 영향 받는 플래그
    pub(super) flag_affected: String,
    /// 오류
    pub(super) exceptions: HashMap<String, Vec<String>>,
    /// c/c++ 대체함수
    pub(super) c_and_cpp_equivalent: Vec<String>,
}

impl Instruction {
    /// Parse instruction name
    pub(super) fn get_instructions_name(&self) -> Vec<String> {
        let data = self.title.clone();
        if data.contains('[') {
            let datas = Self::match_if_possible(data);
            let mut result = Vec::new();
            for data in datas {
                result.append(&mut Self::split_instruction_name_if_possible(data));
            }
            result
        } else {
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
                    result.push(first);
                } else {
                    result.push(item);
                }
            }
            result
        } else {
            [data.to_owned()].into()
        }
    }
    /// Instruction to result string
    pub(super) fn into_md(self) -> Vec<String> {
        let mut result = Vec::new();

        // 제목
        result.push(format!("# {title}", title = self.title));

        // 요약
        result.push("".to_owned());
        result.push(format!("{summary}", summary = self.summary));

        // 인스트럭션 별 설명
        // TODO

        // 문서 하이퍼링크
        // TODO

        // 설명
        let mut description = self.get_description();
        if !description.is_empty() {
            result.push("".to_owned());
            result.append(&mut description);
        }
        drop(description);

        // 호환성
        // TODO

        // 옵코드
        // TODO

        // Flags affected
        if !self.flag_affected.is_empty() {
            result.push("".to_owned());
            result.push("## Flags affected".to_owned());
            result.push("".to_owned());
            result.push(format!(
                "- {flag_affected}",
                flag_affected = self.flag_affected
            ));
        }

        // 예외
        if !self.exceptions.is_empty() {
            result.push("".to_owned());
            result.push("## Exceptions".to_owned());
            result.push("".to_owned());
            for (kind, exceptions) in &self.exceptions {
                result.push(format!("- {kind}"));
                for exception in exceptions {
                    if exception.contains('\u{1}') {
                        let mut iter = exception.split('\u{1}');
                        let head = iter.next().unwrap().trim().to_owned();
                        let tail = iter.next().unwrap().trim().to_owned();
                        result.push(format!("  - {head} - {tail}"));
                    } else {
                        result.push(format!("  > {exception}"));
                    }
                }
            }
        }

        // C/C++ 코드
        if !self.operation.is_empty() {
            result.push("".to_owned());
            result.push("## Operation".to_owned());
            result.push("".to_owned());
            result.push("```C".to_owned());
            result.append(&mut self.get_reformed_operation());
            result.push("```".to_owned());
        }

        // 대체 함수
        // TODO

        result.push("".to_owned());

        // DEBUG PRINT
        result.push("```rust".into());
        result.push(format!("{:#?}", self));
        result.push("```".into());
        result.push("".to_owned());
        result
    }

    fn get_description(&self) -> Vec<String> {
        let mut result = Vec::new();
        let long = self.description.join("");

        for line in long.split(". ") {
            result.push(line.to_owned() + ".");
        }
        result.last_mut().map(|x| {
            x.pop();
        });

        result
    }
    fn get_reformed_operation(&self) -> Vec<String> {
        let data = self.operation.clone();
        [data].into()
    }
}
