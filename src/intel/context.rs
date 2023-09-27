use super::{category::Category, result::Instruction};

pub(super) struct ParsingContent {
    pub(super) result: Vec<Instruction>,
    line: String,
    pub(super) last_category: Category,
    title: String,
    pub(super) stacked_content: Vec<String>,
    pub(super) instruction: Instruction,
}

impl Default for ParsingContent {
    fn default() -> Self {
        Self {
            result: Vec::new(),
            line: String::new(),
            last_category: Category::NeedIgnore,
            title: String::new(),
            stacked_content: Vec::new(),
            instruction: Instruction::default(),
        }
    }
}

impl ParsingContent {
    /// iterator로부터 한 줄을 읽는다.
    pub(super) fn read(&mut self, iter: &mut impl Iterator<Item = String>) {
        self.line = iter.next().unwrap();
    }
    /// 읽은 라인을 가져온다.
    pub(super) fn line(&self) -> &str {
        &self.line
    }
    /// 현재 설정중인 인스트럭션의 제목을 지정한다.
    pub(super) fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }
    /// 라인 읽어온 내요을 파싱할 수 없으므로, 스택에 저장한다.
    pub(super) fn stack(&mut self) {
        self.stacked_content.push(self.line.clone());
    }
    /// 이전까지 파싱했던 내용을 저장한다.
    pub(super) fn clear_stacked_status(&mut self) {
        match self.last_category {
            Category::OpcodeDescription => {
                let content = self.stacked_content.join("");
                self.stacked_content.clear();
                // instruction에 저장
                // TODO
            }
            Category::OpcodeDescriptionStart => {
                // DescriptionStart가 왔으면 end가 올때까지 무시
                // instruction.instruction...
            }
            Category::InstructionOperandEncoding => {
                // 파싱 계획 없음
                self.stacked_content.clear();
            }
            Category::Description => {
                self.instruction.description = self.stacked_content.clone();
                self.stacked_content.clear();
            }
            Category::Operation => {
                self.instruction.operation = self.stacked_content.join("");
                self.stacked_content.clear();
            }
            Category::FlagsAffected => {
                self.instruction.flag_affected = self.stacked_content.join("");
                self.stacked_content.clear();
            }
            Category::Exceptions => {
                // stacked_content를 다른것으로 파싱
                self.instruction
                    .exceptions
                    .insert(self.title.clone(), self.stacked_content.clone());
                self.stacked_content.clear();
            }
            Category::IntrinsicEquivalent => {
                self.instruction.c_and_cpp_equivalent = self.stacked_content.clone();
                self.stacked_content.clear();
            }

            Category::NeedIgnore => {}
            _ => unreachable!(),
        }
    }
}
