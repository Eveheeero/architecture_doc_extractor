use eframe::egui::{self, FontId, RichText};

pub(super) fn main() {
    let _ = eframe::run_native(
        "Pdf Inspector",
        Default::default(),
        Box::new(|cc| Ok(Box::new(PdfInspector::new(cc)))),
    );
}

struct PdfInspector {
    box_list: Vec<(egui::Rect, egui::Color32)>,
    text_list: Vec<InspectorText>,
    // path, enabled
    pdf_path: String,
    pdf_doc: Option<lopdf::Document>,
    pdf_page: String,

    top_panel_height: f32,
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
            box_list: Vec::new(),
            text_list: Vec::new(),
            pdf_path: std::env::current_dir().unwrap().to_str().unwrap().into(),
            pdf_doc: None,
            pdf_page: "0".into(),
            top_panel_height: 0.0,
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
    fn clean(&mut self) {
        self.box_list.clear();
        self.text_list.clear();
    }
    fn load_pdf(&mut self) {
        if self.pdf_doc.is_none() {
            let Ok(doc) = lopdf::Document::load(&self.pdf_path) else {
                self.pdf_page = "Pdf not found".into();
                return;
            };
            self.pdf_doc = Some(doc);
            return;
        }

        let Ok(page) = self.pdf_page.parse::<u32>() else {
            self.pdf_page = "Page not correct".into();
            return;
        };
        self.clean();
        let doc = self.pdf_doc.as_ref().unwrap();
        let contents = crate::pdf::get_page_contents(&doc, page);
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
        let top_panel = egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_enabled(
                    self.pdf_doc.is_none(),
                    egui::TextEdit::singleline(&mut self.pdf_path),
                );
                ui.add_sized(
                    [60.0, 0.0],
                    egui::TextEdit::singleline(&mut self.pdf_page).hint_text("page"),
                );
                if ui.button("Load").clicked() {
                    self.load_pdf();
                }
                if ui.button("Clean").clicked() {
                    self.clean();
                }
                if ui.button("Default").clicked() {
                    self.clean();
                    self.generate_test_object();
                }
                if let Some(mut mouse_position) = mouse_position {
                    if mouse_position.y >= self.top_panel_height {
                        mouse_position.y -= self.top_panel_height;
                        ui.label(format!("mouse position {:?}", mouse_position));
                    }
                }
            });
        });
        self.top_panel_height = top_panel.response.rect.height();

        egui::CentralPanel::default().show(ctx, |ui| {
            for text in &self.text_list {
                let mut rect = text.rect;
                rect.min.y += self.top_panel_height;
                rect.max.y += self.top_panel_height;
                ui.painter().rect_filled(text.rect, 0, text.rect_color);
                let rich_text =
                    RichText::new(&text.text).font(FontId::proportional(text.text_size));
                ui.put(text.rect, egui::Label::new(rich_text));
            }

            for r#box in &self.box_list {
                let mut rect = r#box.0;
                rect.min.y += self.top_panel_height;
                rect.max.y += self.top_panel_height;
                ui.painter().rect_filled(r#box.0, 0, r#box.1);
            }
        });
    }
}
