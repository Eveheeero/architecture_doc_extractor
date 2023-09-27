use super::{category::Category, result::Instruction};

pub(super) struct ParsingContext {
    /// 현재까지 파싱된 인스트러션
    result: Vec<Instruction>,
    /// 파싱중인 라인
    line: String,
    /// 이전 파싱 카테고리
    pub(super) last_category: Category,
    /// 파싱할 수 없어 임시로 저장된 내용
    stacked_content: Vec<String>,
    pub(super) instruction: Instruction,
}

impl Default for ParsingContext {
    fn default() -> Self {
        Self {
            result: Vec::new(),
            line: String::new(),
            last_category: Category::NeedIgnore,
            stacked_content: Vec::new(),
            instruction: Instruction::default(),
        }
    }
}

impl ParsingContext {
    /// iterator로부터 한 줄을 읽는다.
    pub(super) fn read(&mut self, iter: &mut impl Iterator<Item = String>) {
        self.line = iter.next().unwrap();
    }
    /// 읽은 라인을 가져온다.
    pub(super) fn line(&self) -> &str {
        &self.line
    }
    /// 라인 읽어온 내용을 파싱할 수 없으므로, 스택에 저장한다.
    pub(super) fn stack(&mut self, data: Option<String>) {
        if let Some(data) = data {
            self.stacked_content.push(data);
        } else {
            self.stacked_content.push(self.line.clone());
        }
    }
    /// 스택에 저장된 내용을 가져온다.
    pub(super) fn clear_stacked_data(&mut self) -> Vec<String> {
        std::mem::take(&mut self.stacked_content)
    }
    /// 기존 파싱했던 이번 인스트럭션 내용을 저장한다.
    pub(super) fn next_instruction(&mut self) {
        let instruction = std::mem::take(&mut self.instruction);
        self.result.push(instruction);
    }
    /// 파싱을 종료하고 결과값을 반환한다.
    pub(super) fn done(self) -> Vec<Instruction> {
        self.result
    }
}
