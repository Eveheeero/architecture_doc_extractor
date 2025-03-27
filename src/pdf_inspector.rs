use crate::pdf::PDF_TEXT_HEIGHT_FACTOR;
use eframe::egui::{self, FontId, RichText};
use lopdf::Object;
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
    text_background: bool,
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
    selected: bool,
    widget_real_rect: egui::Rect,
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
            paint_page: PdfInspectorPaintPage {
                text_background: true,
                ..Default::default()
            },
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
    fn load_pdf_paint(&mut self, ctx: &egui::Context) {
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
        let extracted = extract_page(contents, ctx);
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
        egui::Window::new("paint_window")
            .scroll([false, true])
            .default_open(false)
            .show(ctx, |ui| {
                let mut is_first = true;
                for text in &self.paint_page.text_list {
                    if !text.selected {
                        continue;
                    }
                    if is_first {
                        is_first = false;
                    } else {
                        ui.separator();
                    }

                    ui.label(format!(
                        "pos {:?} real {:?} {}",
                        text.rect, text.widget_real_rect, &text.text
                    ));
                }
            });

        let window_y = ctx.screen_rect().height();
        egui::CentralPanel::default().show(ctx, |ui| {
            for text in &mut self.paint_page.text_list {
                let mut rect = text.rect;
                (rect.min.y, rect.max.y) = (window_y - rect.max.y, window_y - rect.min.y);
                if self.paint_page.text_background {
                    ui.painter().rect_filled(rect, 0, text.rect_color);
                }
                let rich_text =
                    RichText::new(&text.text).font(FontId::proportional(text.text_size));
                let putted = ui.put(rect, egui::Label::new(rich_text).selectable(false));
                text.widget_real_rect = putted.rect;
                (text.widget_real_rect.min.y, text.widget_real_rect.max.y) = (
                    window_y - text.widget_real_rect.max.y,
                    window_y - text.widget_real_rect.min.y,
                );
            }

            for r#box in &self.paint_page.box_list {
                let mut rect = r#box.0;
                (rect.min.y, rect.max.y) = (window_y - rect.max.y, window_y - rect.min.y);
                ui.painter().rect_filled(rect, 0, r#box.1);
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
        text: impl Into<String>,
        text_size: f32,
        left_top: impl Into<egui::Pos2>,
        right_bottom: impl Into<egui::Pos2>,
    ) -> Self {
        let rect_color = rand_color();
        Self {
            text: text.into(),
            text_size,
            rect: egui::Rect {
                min: left_top.into(),
                max: right_bottom.into(),
            },
            rect_color,
            widget_real_rect: egui::Rect::ZERO,
            selected: true,
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
                    self.load_pdf_paint(ctx);
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
                if self.page == PdfInspectorPage::Paint {
                    ui.checkbox(&mut self.paint_page.text_background, "Box");
                    if let Some(mut mouse_position) = mouse_position {
                        if mouse_position.y >= self.top_panel_height {
                            let window_y = ctx.screen_rect().height();
                            let mouse_height = window_y - mouse_position.y;
                            mouse_position.y -= self.top_panel_height;
                            ui.label(format!(
                                "mouse position {:?}(-{})",
                                mouse_position, mouse_height
                            ));
                        }
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
    ctx: &egui::Context,
) -> (Vec<InspectorText>, Vec<(egui::Rect, egui::Color32)>) {
    let box_list = Vec::new();
    let mut text_list = Vec::new();

    let operations: Vec<lopdf::content::Operation> = page.operations;
    let mut pointer = (0.0, 0.0);
    let mut text_width = 0.0;
    let mut text_height = 0.0;
    for operation in operations {
        let operator = operation.operator;
        let operands = operation.operands;
        match operator.as_ref() {
            "Tm" | "Tlm" => {
                if num(&operands[0]) == num(&operands[3])
                    && num(&operands[1]) == 0.0
                    && num(&operands[2]) == 0.0
                {
                    pointer = (num(&operands[4]), num(&operands[5]));
                }
                text_height = num(&operands[3]);
                text_width = num(&operands[0]);
            }
            "Td" | "TD" => {
                pointer.0 += num(&operands[0]) * text_width;
                pointer.1 += num(&operands[1]) * text_height;
            }
            "T*" => {
                pointer.1 -= text_height * PDF_TEXT_HEIGHT_FACTOR;
            }
            "Tj" | "TJ" => {
                for operand in operands {
                    match operand {
                        Object::String(string, _) => {
                            if text_height != text_width {
                                panic!();
                            }
                            let text = String::from_utf8_lossy(&string);
                            let text_width = calc_text_width(&text, text_width, ctx);
                            text_list.push(InspectorText::new(
                                text,
                                text_height,
                                [pointer.0, pointer.1 - text_height],
                                [pointer.0 + text_width, pointer.1],
                            ));
                        }
                        Object::Array(operands) => {
                            let mut last_x = pointer.0;
                            for operand in operands {
                                match operand {
                                    Object::Integer(i) => last_x -= i as f32 / 1000.0 * text_width,
                                    Object::Real(i) => last_x -= i / 1000.0 * text_width,
                                    Object::String(string, _) => {
                                        if text_height != text_width {
                                            panic!();
                                        }
                                        let text = String::from_utf8_lossy(&string);
                                        let text_width = calc_text_width(&text, text_width, ctx);
                                        text_list.push(InspectorText::new(
                                            text,
                                            text_height,
                                            [last_x, pointer.1 - text_height],
                                            [last_x + text_width, pointer.1],
                                        ));
                                        last_x += text_width;
                                    }
                                    _ => panic!("{:?}", operand),
                                }
                            }
                        }
                        Object::Real(_) | Object::Integer(_) => panic!(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    for text in &text_list {
        println!("{:?} {}", text.rect, text.text);
    }
    (text_list, box_list)
}

fn num(obj: &Object) -> f32 {
    match obj {
        Object::Integer(o) => *o as f32,
        Object::Real(o) => *o,
        _ => unimplemented!(),
    }
}

fn calc_text_width(text: impl AsRef<str>, font_width: f32, ctx: &egui::Context) -> f32 {
    ctx.fonts(|fonts| {
        let font_id = FontId::proportional(font_width);
        let mut width = 0.0;
        for c in text.as_ref().chars() {
            width += fonts.glyph_width(&font_id, c);
        }
        width
    })
}
