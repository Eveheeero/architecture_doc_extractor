use crate::pdf::v1::{extract_num, PDF_TEXT_HEIGHT_FACTOR};
use either::Either;
use geo::{BoundingRect, MultiPolygon, Rect};
use lopdf::{content::Operation, Object, StringFormat};
use std::cmp::Ordering;
use tracing::debug;

pub fn operator_to_chars(
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
                font = Some(fonts.get(op.operands[0].as_name_str().unwrap()));
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
                        Object::String(s, StringFormat::Literal) => {
                            for c in s {
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
                        Object::String(s, StringFormat::Hexadecimal) => {
                            debug!(?s, "Hex in Tj");
                        }
                        Object::Array(operands) => {
                            for operand in operands {
                                match operand {
                                    Object::Integer(i) => {
                                        last_x -= i as f32 / 1000.0 * width_factor
                                    }
                                    Object::Real(i) => last_x -= i / 1000.0 * width_factor,
                                    Object::String(s, StringFormat::Literal) => {
                                        for c in s {
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
                                    Object::String(s, StringFormat::Hexadecimal) => {
                                        debug!(?s, "Hex in Tj");
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
pub fn detect_strings(mut cs: Vec<PdfChar>) -> Vec<PdfString> {
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

pub fn sort_strings(d: &mut Vec<PdfString>) {
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

pub struct PdfString(Vec<PdfChar>);
impl PdfString {
    #[inline]
    pub fn get(&self) -> String {
        self.0.iter().map(PdfChar::get).collect()
    }
    pub fn rect(&self) -> Rect<f32> {
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
pub struct PdfChar {
    raw: Either<u8, Box<[u8; 2]>>,
    // x, height
    rect: Rect<f32>,
    represent_as: Option<String>,
}
impl PdfChar {
    pub fn make_ready(&mut self) {
        if self.represent_as.is_some() {
            return;
        }
        match &self.raw {
            Either::Left(raw) => {
                let data = match raw {
                    0x20..=0x7e => String::from_utf8([*raw].into()).unwrap(),
                    0x2 => '≠'.into(),
                    0x3 => '≥'.into(),
                    0x4 => '≤'.into(),
                    0x5 => '*'.into(),
                    0x6 => '∞'.into(),
                    0x7 => 'π'.into(),
                    0x8 => 'Š'.into(),
                    0x9 => 'ε'.into(),
                    0xa => 'Σ'.into(),
                    0xb => 'σ'.into(),
                    0x82 => ','.into(),
                    0x87 => '⁄'.into(),
                    0x85 => '…'.into(),
                    0x8a => '-'.into(),
                    0x8f => '≠'.into(),
                    0x91 => '\''.into(),
                    0x92 => '\''.into(),
                    0x93 => '\"'.into(),
                    0x94 => '\"'.into(),
                    0x95 => '-'.into(),
                    0x96 => '-'.into(),
                    0x97 => '-'.into(),
                    0x99 => '™'.into(),
                    0xab => "<<".into(),
                    0xae => '®'.into(),
                    0xb1 => '±'.into(),
                    0xb5 => 'μ'.into(),
                    0xbd => '½'.into(),
                    _ => unimplemented!("{}", raw),
                };
                self.represent_as = Some(data);
            }
            Either::Right(raw) => {
                let data = match raw.as_ref() {
                    [0, 0] => "".into(),
                    _ => unimplemented!("{:?}", raw),
                };
                self.represent_as = Some(data);
            }
        }
    }
    #[inline]
    pub fn get(&self) -> &str {
        self.represent_as.as_ref().expect("make_ready not called")
    }
}

pub fn operator_to_boxes(data: impl IntoIterator<Item = Operation>) -> PdfBoxes {
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
                assert!(op.operands.len() == 4);
                let [x, y, w, h] = [
                    extract_num(&op.operands[0]),
                    extract_num(&op.operands[1]),
                    extract_num(&op.operands[2]),
                    extract_num(&op.operands[3]),
                ];
                rect = Rect::new([x, y], [x + w, y + h]);
            }
            _ => {}
        }
    }

    PdfBoxes {
        lines: result,
        cells: None,
    }
}

#[derive(Debug, Clone)]

pub struct PdfBoxes {
    lines: Vec<PdfBox>,
    cells: Option<Vec<Rect<f32>>>,
}

#[derive(Debug, Clone)]
pub struct PdfBox {
    id: usize,
    rect: Rect<f32>,
}

impl PdfBoxes {
    pub fn get_lines(&self) -> &Vec<PdfBox> {
        &self.lines
    }
    pub fn get_cells(&self) -> &Vec<Rect<f32>> {
        self.cells.as_ref().unwrap()
    }
    /// 주어진 lines로 어떤 셀이 만들어졌는지 연산
    pub fn prepare_cells(&mut self) {
        if self.cells.is_some() {
            return;
        }

        let mut result = Vec::new();

        let mut horizontal_lines = self
            .lines
            .iter()
            .filter(|x| x.rect.width() > x.rect.height())
            .map(|x| x.rect)
            .collect::<Vec<_>>();
        horizontal_lines.sort_by(|a, b| a.min().y.partial_cmp(&b.min().y).unwrap());
        let mut vertical_lines = self
            .lines
            .iter()
            .filter(|x| x.rect.height() > x.rect.width())
            .map(|x| x.rect)
            .collect::<Vec<_>>();
        vertical_lines.sort_by(|a, b| a.min().x.partial_cmp(&b.min().x).unwrap());

        for top in &horizontal_lines {
            let mut bottom_closer: Option<Rect<f32>> = None;
            let mut left_closer: Option<Rect<f32>> = None;
            let mut right_closer: Option<Rect<f32>> = None;

            // top 아래에 있는 수평선(bottom)들 중 가장 가까운 선부터 순회
            let mut bottoms = horizontal_lines
                .iter()
                .filter(|x| x.max().y < top.min().y)
                .collect::<Vec<_>>();
            bottoms.sort_by(|a, b| a.max().y.partial_cmp(&b.max().y).unwrap().reverse());
            for bottom in bottoms {
                // cell 영역의 x 범위 (top과 bottom의 교집합)
                let min_x = top.min().x.max(bottom.min().x);
                let max_x = top.max().x.min(bottom.max().x);
                // Y 범위를 가로지르는 수직선 후보 및 x 겹침 필터
                let mut vs = vertical_lines
                    .iter()
                    .filter(|v| v.max().y >= top.min().y && v.min().y <= bottom.max().y)
                    .filter(|v| v.max().x >= min_x && v.min().x <= max_x)
                    .collect::<Vec<_>>();
                // 후보선 2개 이상일 때 좌우 경계 설정
                if vs.len() >= 2 {
                    vs.sort_by(|a, b| a.min().x.partial_cmp(&b.min().x).unwrap());
                    left_closer = Some(**vs.first().unwrap());
                    right_closer = Some(**vs.last().unwrap());
                    bottom_closer = Some(*bottom);
                    break;
                }
            }

            if bottom_closer.is_some() && left_closer.is_some() && right_closer.is_some() {
                let left_top = [left_closer.unwrap().max().x, top.min().y];
                let right_bottom = [
                    right_closer.unwrap().min().x,
                    bottom_closer.unwrap().max().y,
                ];
                result.push(Rect::new(left_top, right_bottom));
            }
        }

        self.cells = Some(result);
    }
    pub fn get_wrapping_cell(&self, rect: &Rect<f32>) -> Option<Rect<f32>> {
        use geo::Contains;
        self.cells
            .as_ref()
            .unwrap()
            .iter()
            .find(|r| r.contains(rect))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Rect;
    use lopdf::content::Operation;
    use lopdf::Object;
    /// x y w h
    fn generate_cells(lines: &[[f32; 4]]) -> Vec<Rect<f32>> {
        let mut ops = Vec::new();
        for line in lines {
            ops.push(Operation::new(
                "re",
                make_line(line[0], line[1], line[2], line[3]),
            ));
            ops.push(Operation::new("f", [].into()));
        }
        let mut boxes = operator_to_boxes(ops);
        boxes.prepare_cells();
        boxes.get_cells().clone()
    }
    fn make_line(x: f32, y: f32, w: f32, h: f32) -> Vec<Object> {
        [
            Object::Real(x),
            Object::Real(y),
            Object::Real(w),
            Object::Real(h),
        ]
        .into()
    }

    #[test]
    fn test_prepare_cells_fit() {
        let cells = generate_cells(&[
            [0.0, 100.0, 100.0, 0.0],
            [0.0, 50.0, 100.0, 0.0],
            [0.0, 50.0, 0.0, 50.0],
            [100.0, 50.0, 0.0, 50.0],
        ]);
        assert_eq!(cells.len(), 1);
        let cell = cells[0];
        let expected = Rect::new([0.0, 50.0], [100.0, 100.0]);
        assert_eq!(cell, expected);
    }
    #[test]
    fn test_prepare_cells_thick_line() {
        let cells = generate_cells(&[
            [5.0, 100.0, 95.0, 3.0],
            [5.0, 50.0, 95.0, 4.0],
            [0.0, 54.0, 5.0, 46.0],
            [100.0, 54.0, 6.0, 46.0],
        ]);
        assert_eq!(cells.len(), 1);
        let cell = cells[0];
        let expected = Rect::new([5.0, 54.0], [100.0, 100.0]);
        assert_eq!(cell, expected);
    }
    #[test]
    fn test_prepare_cells_x_unfit_1() {
        let cells = generate_cells(&[
            [5.0, 100.0, 94.9, 3.0], // changed
            [5.0, 50.0, 95.0, 4.0],
            [0.0, 54.0, 5.0, 46.0],
            [100.0, 54.0, 6.0, 46.0],
        ]);
        assert!(cells.is_empty());
    }
    #[test]
    fn test_prepare_cells_x_unfit_2() {
        let cells = generate_cells(&[
            [5.0, 100.0, 95.0, 3.0],
            [5.1, 50.0, 94.9, 4.0], // changed
            [0.0, 54.0, 5.0, 46.0],
            [100.0, 54.0, 6.0, 46.0],
        ]);
        assert!(cells.is_empty());
    }
    #[test]
    fn test_prepare_cells_y_unfit_1() {
        let cells = generate_cells(&[
            [5.0, 100.0, 95.0, 3.0],
            [5.0, 50.0, 95.0, 4.0],
            [0.0, 54.0, 5.0, 46.0],
            [100.0, 54.1, 6.0, 46.0], // changed, right line move a bit higher
        ]);
        assert!(cells.is_empty());
    }
    #[test]
    fn test_prepare_cells_y_unfit_2() {
        let cells = generate_cells(&[
            [5.0, 100.0, 95.0, 3.0],
            [5.0, 50.0, 95.0, 4.0],
            [0.0, 54.0, 5.0, 45.9], // changed
            [100.0, 54.0, 6.0, 46.0],
        ]);
        assert!(cells.is_empty());
    }
}
