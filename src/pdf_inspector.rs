use eframe::egui::{self, FontId, RichText};
use std::collections::BTreeMap;

pub(super) fn main() {
    let _ = eframe::run_native(
        "Pdf Inspector",
        Default::default(),
        Box::new(|cc| Ok(Box::new(PdfInspector::new(cc)))),
    );
}

struct PdfInspector {
    page: PdfInspectorPage,
    paint_page: PdfInspectorPaintPage,
    inspector_page: PdfInspectorInspectorPage,
    pdf_path: String,
    pdf_doc: Option<lopdf::Document>,
    pdf_page: String,

    top_panel_height: f32,
}
#[derive(Default)]
struct PdfInspectorPaintPage {
    text_list: Vec<InspectorText>,
    box_list: Vec<(egui::Rect, egui::Color32)>,
}
#[derive(Default)]
struct PdfInspectorInspectorPage {
    operations: Vec<lopdf::content::Operation>,
    // operator, selected
    operators: BTreeMap<String, (egui::Color32, bool)>,
}
struct InspectorText {
    text: String,
    text_size: f32,
    rect: egui::Rect,
    rect_color: egui::Color32,
}
#[derive(PartialEq, Eq)]
enum PdfInspectorPage {
    Paint,
    Inspector,
}
fn rand_color() -> egui::Color32 {
    egui::Color32::from_rgb(
        fastrand::u8(20..200),
        fastrand::u8(20..200),
        fastrand::u8(20..200),
    )
}
impl PdfInspector {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut result = Self {
            page: PdfInspectorPage::Paint,
            paint_page: Default::default(),
            inspector_page: Default::default(),
            pdf_path: std::env::current_dir()
                .unwrap()
                .join("src")
                .join("intel")
                .join("intel.pdf")
                .to_str()
                .unwrap()
                .into(),
            pdf_doc: None,
            pdf_page: "1".into(),
            top_panel_height: 0.0,
        };
        result.generate_test_object();
        result
    }
    fn generate_test_object(&mut self) {
        self.paint_page.text_list.push(InspectorText::new(
            "test".to_owned(),
            10.0,
            (60.0, 120.0),
            (180.0, 200.0),
        ));
        self.paint_page.text_list.push(InspectorText::new(
            "test2".to_owned(),
            30.0,
            (160.0, 100.0),
            (220.0, 150.0),
        ));
    }
    fn clean(&mut self) {
        self.paint_page.box_list.clear();
        self.paint_page.text_list.clear();
        self.page = PdfInspectorPage::Paint;
    }
    fn load_pdf_paint(&mut self) {
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
        let doc = self.pdf_doc.as_ref().unwrap();
        if page <= 0 || doc.get_pages().len() < page as usize {
            self.pdf_page = "Page not found".into();
            return;
        }
        let contents = crate::pdf::get_page_contents(&doc, page);
        let extracted = extract_page(contents);
        self.paint_page.text_list = extracted.0;
        self.paint_page.box_list = extracted.1;
        self.page = PdfInspectorPage::Paint;
    }
    fn load_pdf_inspect(&mut self) {
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
        let doc = self.pdf_doc.as_ref().unwrap();
        if page <= 0 || doc.get_pages().len() < page as usize {
            self.pdf_page = "Page not found".into();
            return;
        }
        let contents = crate::pdf::get_page_contents(&doc, page);
        let operators: std::collections::HashSet<_> =
            contents.operations.iter().map(|x| &x.operator).collect();
        self.inspector_page.operators = operators
            .into_iter()
            .map(|x| (x.clone(), (rand_color(), true)))
            .collect();
        self.inspector_page.operations = contents.operations;
        self.page = PdfInspectorPage::Inspector;
    }
    fn paint_page(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            for text in &self.paint_page.text_list {
                let mut rect = text.rect;
                rect.min.y += self.top_panel_height;
                rect.max.y += self.top_panel_height;
                ui.painter().rect_filled(text.rect, 0, text.rect_color);
                let rich_text =
                    RichText::new(&text.text).font(FontId::proportional(text.text_size));
                ui.put(text.rect, egui::Label::new(rich_text));
            }

            for r#box in &self.paint_page.box_list {
                let mut rect = r#box.0;
                rect.min.y += self.top_panel_height;
                rect.max.y += self.top_panel_height;
                ui.painter().rect_filled(r#box.0, 0, r#box.1);
            }
        });
    }
    fn inspect_page(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("inspect_side_panel")
            .resizable(false)
            .exact_width(80.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for operator in &mut self.inspector_page.operators {
                        ui.checkbox(&mut operator.1 .1, operator.0.as_str());
                    }
                })
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for op in &self.inspector_page.operations {
                    let operator = &op.operator;
                    let operator_setting = self.inspector_page.operators.get(operator).unwrap();
                    if operator_setting.1 {
                        ui.label(RichText::new(format!("{:?}", op)).color(operator_setting.0));
                    }
                }
            })
        });
    }
}
impl InspectorText {
    fn new(
        text: String,
        text_size: f32,
        left_top: impl Into<egui::Pos2>,
        right_bottom: impl Into<egui::Pos2>,
    ) -> Self {
        let rect_color = rand_color();
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
                if ui.button("Paint").clicked() {
                    self.load_pdf_paint();
                }
                if ui.button("Inspect").clicked() {
                    self.load_pdf_inspect();
                }
                if ui.button("Clean").clicked() {
                    self.clean();
                }
                if ui.button("Default").clicked() {
                    self.clean();
                    self.generate_test_object();
                }
                if let Some(mut mouse_position) = mouse_position {
                    if mouse_position.y >= self.top_panel_height
                        && self.page == PdfInspectorPage::Paint
                    {
                        let window_y = ctx.screen_rect().height();
                        let mouse_height = window_y - mouse_position.y;
                        mouse_position.y -= self.top_panel_height;
                        ui.label(format!(
                            "mouse position {:?}(-{})",
                            mouse_position, mouse_height
                        ));
                    }
                }
            });
        });
        self.top_panel_height = top_panel.response.rect.height();

        if self.page == PdfInspectorPage::Paint {
            self.paint_page(ctx);
        } else if self.page == PdfInspectorPage::Inspector {
            self.inspect_page(ctx);
        }
    }
}

fn extract_page(
    page: lopdf::content::Content,
) -> (Vec<InspectorText>, Vec<(egui::Rect, egui::Color32)>) {
    let mut box_list = Vec::new();
    let mut text_list = Vec::new();
    let operations: Vec<lopdf::content::Operation> = page.operations;
    (text_list, box_list)
}
