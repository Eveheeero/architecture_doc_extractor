use crate::pdf::v1::{extract_num, PDF_TEXT_HEIGHT_FACTOR};
use either::Either;
use geo::{BoundingRect, MultiPolygon, Rect};
use lopdf::{content::Operation, Object};
use std::cmp::Ordering;
use tracing::debug;

pub(crate) fn operator_to_chars(
    fonts: crate::pdf::PdfFonts,
    data: impl IntoIterator<Item = Operation>,
) -> Vec<PdfChar> {
    let mut result = Vec::new();
    let mut font = None;
    let mut font_scale = 1.0;
    let mut word_space = 0.0;
    let mut char_space = 0.0;
    let mut pointer = (0.0, 0.0);
    let mut width_factor = 0.0;
    let mut height_factor = 0.0;
    for op in data.into_iter() {
        match op.operator.as_str() {
            "Tfs" => unimplemented!(),
            "Tf" => {
                font = fonts.get(op.operands[0].as_name_str().unwrap());
                font_scale = extract_num(&op.operands[1]);
            }
            "Tc" => char_space = extract_num(&op.operands[0]),
            "Tw" => word_space = extract_num(&op.operands[0]),
            "T*" => {
                pointer.1 -= height_factor * PDF_TEXT_HEIGHT_FACTOR;
            }
            "Td" | "TD" => {
                pointer.0 += extract_num(&op.operands[0]) * width_factor;
                pointer.1 += extract_num(&op.operands[1]) * height_factor;
            }
            "Tm" | "Tlm" => {
                if extract_num(&op.operands[0]) == extract_num(&op.operands[3])
                    && extract_num(&op.operands[1]) == 0.0
                    && extract_num(&op.operands[2]) == 0.0
                {
                    pointer.0 = extract_num(&op.operands[4]);
                    pointer.1 = extract_num(&op.operands[5]);
                }
                width_factor = extract_num(&op.operands[0]);
                height_factor = extract_num(&op.operands[3]);
            }
            "Tj" | "TJ" => {
                let mut last_x = pointer.0;
                for operand in op.operands {
                    match operand {
                        Object::String(s, m) => {
                            if m == lopdf::StringFormat::Hexadecimal {
                                debug!(?s, "Hex in Tj");
                                continue;
                            }
                            let s = String::from_utf8_lossy(&s);
                            for c in s.chars() {
                                if c == char::REPLACEMENT_CHARACTER {
                                    continue;
                                }
                                let width = font.as_ref().unwrap().get_char_width(c)
                                    * width_factor
                                    * font_scale;
                                let height = height_factor;
                                let _left_bottom = (last_x, pointer.1);
                                let rect = Rect::new(
                                    [last_x, pointer.1],
                                    [last_x + width, pointer.1 + height],
                                );
                                let pdf_char = PdfChar {
                                    raw: Either::Left(c),
                                    rect,
                                    represent_as: None,
                                };
                                last_x += rect.width() + char_space;
                                result.push(pdf_char);
                            }
                            last_x += word_space;
                        }
                        Object::Array(operands) => {
                            for operand in operands {
                                match operand {
                                    Object::Integer(i) => {
                                        last_x -= i as f32 / 1000.0 * width_factor
                                    }
                                    Object::Real(i) => last_x -= i / 1000.0 * width_factor,
                                    Object::String(s, m) => {
                                        if m == lopdf::StringFormat::Hexadecimal {
                                            debug!(?s, "Hex in Tj");
                                            continue;
                                        }
                                        let s = String::from_utf8_lossy(&s);
                                        for c in s.chars() {
                                            if c == char::REPLACEMENT_CHARACTER {
                                                continue;
                                            }
                                            let width = font.as_ref().unwrap().get_char_width(c)
                                                * width_factor
                                                * font_scale;
                                            let height = height_factor;
                                            let _left_bottom = (last_x, pointer.1);
                                            let rect = Rect::new(
                                                [last_x, pointer.1],
                                                [last_x + width, pointer.1 + height],
                                            );
                                            let pdf_char = PdfChar {
                                                raw: Either::Left(c),
                                                rect,
                                                represent_as: None,
                                            };
                                            last_x += rect.width() + char_space;
                                            result.push(pdf_char);
                                        }
                                        last_x += word_space;
                                    }
                                    _ => panic!("{:?}", operand),
                                }
                            }
                        }
                        _ => panic!("{:?}", operand),
                    }
                }
            }
            _ => {}
        }
    }
    result
}
pub(crate) fn detect_strings(mut cs: Vec<PdfChar>) -> Vec<PdfString> {
    cs.iter_mut().for_each(PdfChar::make_ready);
    let nearby = |s: &PdfString, c: &PdfChar| {
        let s = s.rect();
        let c = c.rect;
        let x_distance = (s.center().x - c.center().x).abs();
        let y_distance = (s.center().y - c.center().y).abs();
        (s.width() + c.width()) / 2.0 + 20.0 >= x_distance && s.height() * 1.0 / 3.0 >= y_distance
    };
    let mut result = Vec::new();
    while let Some(c) = cs.pop() {
        let Some(s) = result.iter_mut().find(|s| nearby(s, &c)) else {
            result.push(PdfString([c].into()));
            continue;
        };
        let position =
            s.0.iter()
                .position(|sc| sc.rect.center().x > c.rect.center().x);
        if let Some(position) = position {
            if position != 0 {
                if let Some(before_y) = s.0.get(position - 1).map(|before| before.rect.center().y) {
                    if before_y + 10.0 < c.rect.center().y {
                        todo!("represent as 특수문자로 변경");
                    }
                }
            }
            s.0.insert(position, c);
        } else {
            if let Some(before_y) = s.0.last().map(|before| before.rect.center().y) {
                if before_y + 10.0 < c.rect.center().y {
                    todo!("represent as 특수문자로 변경");
                }
            }
            s.0.push(c);
        }
    }
    result
}

pub(crate) fn sort_strings(d: &mut Vec<PdfString>) {
    d.sort_by(|a, b| {
        let height = a
            .rect()
            .center()
            .y
            .partial_cmp(&b.rect().center().y)
            .unwrap();
        if height == Ordering::Equal {
            let x = a
                .rect()
                .center()
                .x
                .partial_cmp(&b.rect().center().x)
                .unwrap();
            x
        } else {
            height.reverse()
        }
    });
}

pub(crate) struct PdfString(Vec<PdfChar>);
impl PdfString {
    pub(crate) fn get(&self) -> String {
        self.0.iter().map(PdfChar::get).collect()
    }
    pub(crate) fn rect(&self) -> Rect<f32> {
        if self.0.is_empty() {
            panic!("no rect")
        }
        let mut polygons = Vec::new();
        for c in &self.0 {
            let rect = c.rect;
            let polygon = rect.to_polygon();
            polygons.push(polygon);
        }
        let multipolygon = MultiPolygon::new(polygons);
        multipolygon.bounding_rect().unwrap()
    }
}
pub(crate) struct PdfChar {
    raw: Either<char, u8>,
    // x, height
    rect: Rect<f32>,
    represent_as: Option<String>,
}
impl PdfChar {
    pub(crate) fn make_ready(&mut self) {
        if self.represent_as.is_some() {
            return;
        }
        if self.raw.is_left() {
            self.represent_as = Some(self.raw.left().unwrap().to_string());
            return;
        }
        let data = self.raw.right().unwrap();
        let data = match data {
            0x92 => '\''.into(),
            0x93 => '\"'.into(),
            0x94 => '\"'.into(),
            0x95 => '-'.into(),
            0x96 => '-'.into(),
            0x97 => '-'.into(),
            0x8a => '-'.into(),
            _ => unimplemented!("{}", data),
        };
        self.represent_as = Some(data);
    }
    pub(crate) fn get(&self) -> &str {
        self.represent_as.as_ref().expect("make_ready not called")
    }
}

pub(crate) fn operator_to_boxes(data: impl IntoIterator<Item = Operation>) -> PdfBoxes {
    let mut result = Vec::new();

    let mut rect = Rect::new([0.0, 0.0], [0.0, 0.0]);
    for op in data.into_iter() {
        match op.operator.as_str() {
            "f" | "F" => {
                result.push(PdfBox {
                    id: result.len(),
                    rect,
                });
            }
            "re" => {
                let [x, y, w, h] = op
                    .operands
                    .iter()
                    .map(|o| o.as_f32().unwrap())
                    .collect::<Vec<_>>()[..]
                else {
                    panic!()
                };
                rect = Rect::new([x, y], [x + w, y + h]);
            }
            _ => {}
        }
    }

    PdfBoxes(result)
}

pub(crate) struct PdfBoxes(Vec<PdfBox>);
pub(crate) struct PdfBox {
    id: usize,
    rect: Rect<f32>,
}

impl PdfBoxes {
    const X_NEGINF: [f32; 2] = [f32::NEG_INFINITY, 0.0];
    const X_INF: [f32; 2] = [f32::INFINITY, 0.0];
    const Y_INF: [f32; 2] = [0.0, f32::INFINITY];
    const Y_NEGINF: [f32; 2] = [0.0, f32::NEG_INFINITY];
    /// 주어진 rect가 어떤 셀에 속하는지 연산 후 반환
    pub(crate) fn get_surrounding_rect(&self, rect: Rect<f32>) -> Option<Rect<f32>> {
        let mut left_closer = Rect::new(Self::X_NEGINF, Self::X_NEGINF);
        let mut right_closer = Rect::new(Self::X_INF, Self::X_INF);
        let mut top_closer = Rect::new(Self::Y_INF, Self::Y_INF);
        let mut bottom_closer = Rect::new(Self::Y_NEGINF, Self::Y_NEGINF);

        for r#box in self.0.iter() {
            let r = r#box.rect;
            // top
            if r.min().y >= rect.max().y && top_closer.min().y > r.max().y {
                top_closer = r;
            }
            // bottom
            if r.max().y <= rect.min().y && bottom_closer.max().y < r.min().y {
                bottom_closer = r;
            }
            // left
            if r.max().x <= rect.min().x && left_closer.max().x < r.min().x {
                left_closer = r;
            }
            // right
            if r.min().x >= rect.max().x && right_closer.min().x > r.max().x {
                right_closer = r;
            }
        }

        if left_closer == Rect::new(Self::X_NEGINF, Self::X_NEGINF)
            || right_closer == Rect::new(Self::X_INF, Self::X_INF)
            || top_closer == Rect::new(Self::Y_INF, Self::Y_INF)
            || bottom_closer == Rect::new(Self::Y_NEGINF, Self::Y_NEGINF)
        {
            return None;
        }
        let left_top = [left_closer.max().x, top_closer.min().y];
        let right_bottom = [right_closer.min().x, bottom_closer.max().y];
        Some(Rect::new(left_top, right_bottom))
    }
    pub(crate) fn prepare_cells(&self) -> Vec<Rect<f32>> {
        let mut result = Vec::new();

        let mut horizontal_lines = self
            .0
            .iter()
            .filter(|x| x.rect.width() > x.rect.height())
            .map(|x| x.rect)
            .collect::<Vec<_>>();
        horizontal_lines.sort_by(|a, b| a.min().y.partial_cmp(&b.min().y).unwrap());
        let mut vertical_lines = self
            .0
            .iter()
            .filter(|x| x.rect.height() > x.rect.width())
            .map(|x| x.rect)
            .collect::<Vec<_>>();
        vertical_lines.sort_by(|a, b| a.min().x.partial_cmp(&b.min().x).unwrap());

        for top in horizontal_lines {
            let mut bottom_closer: Option<Rect<f32>> = None;
            let mut left_closer: Option<Rect<f32>> = None;
            let mut right_closer: Option<Rect<f32>> = None;

            todo!();

            if bottom_closer.is_some() && left_closer.is_some() && right_closer.is_some() {
                let left_top = [left_closer.unwrap().max().x, top.min().y];
                let right_bottom = [
                    right_closer.unwrap().min().x,
                    bottom_closer.unwrap().max().y,
                ];
                result.push(Rect::new(left_top, right_bottom));
            }
        }

        result
    }
}
