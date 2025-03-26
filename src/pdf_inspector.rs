use eframe::egui::{self, FontId, RichText};

pub(super) fn main() {
    let _ = eframe::run_native(
        "Pdf Inspector",
        Default::default(),
        Box::new(|cc| Ok(Box::new(PdfInspector::new(cc)))),
    );
}

struct PdfInspector {
    text_list: Vec<InspectorText>,
}
struct InspectorText {
    text: String,
    text_size: f32,
    rect: egui::Rect,
    rect_color: egui::Color32,
}
impl PdfInspector {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut result = Self {
            text_list: Vec::new(),
        };
        result.generate_test_object();
        result
    }
    fn generate_test_object(&mut self) {
        self.text_list.push(InspectorText::new(
            "test".to_owned(),
            10.0,
            (60.0, 120.0),
            (180.0, 200.0),
        ));
        self.text_list.push(InspectorText::new(
            "test2".to_owned(),
            30.0,
            (160.0, 100.0),
            (220.0, 150.0),
        ));
    }
}
impl InspectorText {
    fn new(
        text: String,
        text_size: f32,
        left_top: impl Into<egui::Pos2>,
        right_bottom: impl Into<egui::Pos2>,
    ) -> Self {
        let rect_color = egui::Color32::from_rgb(
            fastrand::u8(20..200),
            fastrand::u8(20..200),
            fastrand::u8(20..200),
        );
        Self {
            text,
            text_size,
            rect: egui::Rect {
                min: left_top.into(),
                max: right_bottom.into(),
            },
            rect_color,
        }
    }
}
impl eframe::App for PdfInspector {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mouse_position = ctx.input(|i| i.pointer.hover_pos());
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(mouse_position) = mouse_position {
                ui.label(format!("mouse position {:?}", mouse_position));
            }

            for text in &self.text_list {
                ui.painter().rect_filled(text.rect, 0, text.rect_color);
                let rich_text =
                    RichText::new(&text.text).font(FontId::proportional(text.text_size));
                ui.put(text.rect, egui::Label::new(rich_text));
            }
        });
    }
}
