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
            "Tfs" => {
                        tracing::warn!("Tfs operator not supported, skipping");
                    }
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
                                    font_scale: width_factor,
                                    is_superscript: false,
                                    is_subscript: false,
                                    represent_as: None,
                                };
                                last_x += rect.width() + char_space;
                                result.push(pdf_char);
                            }
                            last_x += word_space;
                        }
                        Object::String(s, StringFormat::Hexadecimal) => {
                            debug!(?s, "Hex in Tj");
                            for c_hex in s.chunks(2) {
                                let c_hex = [c_hex[0], c_hex[1]];
                                let c = font.as_ref().unwrap().get_cid_char(c_hex);
                                let width = font.as_ref().unwrap().get_cid_width(c_hex)
                                    * width_factor
                                    * font_scale;
                                let height = height_factor;
                                let rect = Rect::new(
                                    [last_x, pointer.1],
                                    [last_x + width, pointer.1 + height],
                                );
                                let pdf_char = PdfChar {
                                    raw: Either::Right((c_hex.into(), c)),
                                    rect,
                                    font_scale: width_factor,
                                    is_superscript: false,
                                    is_subscript: false,
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
                                                font_scale: width_factor,
                                                is_superscript: false,
                                                is_subscript: false,
                                                represent_as: None,
                                            };
                                            last_x += rect.width() + char_space;
                                            result.push(pdf_char);
                                        }
                                        last_x += word_space;
                                    }
                                    Object::String(s, StringFormat::Hexadecimal) => {
                                        debug!(?s, "Hex in Tj");
                                        for c_hex in s.chunks(2) {
                                            let c_hex = [c_hex[0], c_hex[1]];
                                            let c = font.as_ref().unwrap().get_cid_char(c_hex);
                                            let width = font.as_ref().unwrap().get_cid_width(c_hex)
                                                * width_factor
                                                * font_scale;
                                            let height = height_factor;
                                            let rect = Rect::new(
                                                [last_x, pointer.1],
                                                [last_x + width, pointer.1 + height],
                                            );
                                            let pdf_char = PdfChar {
                                                raw: Either::Right((c_hex.into(), c)),
                                                rect,
                                                font_scale: width_factor,
                                                is_superscript: false,
                                                is_subscript: false,
                                                represent_as: None,
                                            };
                                            last_x += rect.width() + char_space;
                                            result.push(pdf_char);
                                        }
                                        last_x += word_space;
                                    }
                                    _ => {
                                        tracing::warn!(?operand, "unexpected operand in TJ array, skipping");
                                    }
                                }
                            }
                        }
                        _ => {
                            tracing::warn!(?operand, "unexpected operand in Tj/TJ, skipping");
                        }
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
            s.0.insert(position, c);
        } else {
            s.0.push(c);
        }
    }
    // Per-line baseline detection and superscript/subscript marking
    for s in &mut result {
        mark_super_subscripts(s);
        for c in &mut s.0 {
            c.apply_super_subscript();
        }
    }
    result
}

/// Compute per-string baseline (median Y of dominant font_scale chars),
/// then mark chars with significant Y offset as superscript/subscript.
fn mark_super_subscripts(s: &mut PdfString) {
    if s.0.len() < 2 {
        return;
    }
    // Find the dominant font_scale by counting occurrences (bucketed by ±0.5)
    let dominant_scale = {
        let mut buckets: Vec<(f32, usize)> = Vec::new();
        for c in &s.0 {
            if let Some(b) = buckets.iter_mut().find(|(s, _)| (*s - c.font_scale).abs() < 0.5) {
                b.1 += 1;
            } else {
                buckets.push((c.font_scale, 1));
            }
        }
        buckets
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(scale, _)| scale)
            .unwrap_or(0.0)
    };

    // Collect Y centers of chars matching dominant scale
    let mut baseline_ys: Vec<f32> = s
        .0
        .iter()
        .filter(|c| (c.font_scale - dominant_scale).abs() < 0.5)
        .map(|c| c.rect.center().y)
        .collect();

    if baseline_ys.is_empty() {
        return;
    }

    // Compute median Y as baseline
    baseline_ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let baseline = baseline_ys[baseline_ys.len() / 2];

    // Threshold: fraction of dominant font scale
    let threshold = dominant_scale * 0.2;

    for c in &mut s.0 {
        // Only chars with a smaller font_scale than dominant are true super/subscripts.
        // Same-scale or larger-scale chars with Y offset are bullets or alignment artifacts.
        if c.font_scale >= dominant_scale - 0.5 {
            continue;
        }
        let offset = c.rect.center().y - baseline;
        if offset.abs() > threshold {
            // In PDF coordinates, larger Y = higher on page
            if offset > 0.0 {
                c.is_superscript = true;
            } else {
                c.is_subscript = true;
            }
        }
    }
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
    pub fn font_scale(&self) -> f32 {
        self.0.first().map(|c| c.font_scale).unwrap_or(0.0)
    }
    pub fn x(&self) -> f32 {
        self.rect().min().x
    }
    pub fn chars(&self) -> &[PdfChar] {
        &self.0
    }
}
pub struct PdfChar {
    raw: Either<u8, (Box<[u8; 2]>, char)>,
    // x, height
    pub rect: Rect<f32>,
    font_scale: f32,
    is_superscript: bool,
    is_subscript: bool,
    represent_as: Option<String>,
}
impl PdfChar {
    pub fn make_ready(&mut self) {
        if self.represent_as.is_some() {
            return;
        }
        let data = match &self.raw {
            Either::Left(raw) => match raw {
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
                0xc => '√'.into(),
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
                0xb2 => '²'.into(),
                0xb3 => '³'.into(),
                0xb5 => 'μ'.into(),
                0xbc => '¼'.into(),
                0xbd => '½'.into(),
                0xd7 => '×'.into(),
                0xf7 => '÷'.into(),
                _ => {
                    tracing::warn!(byte = raw, "unmapped byte in make_ready, using replacement char");
                    '\u{FFFD}'.into()
                }
            },
            Either::Right(raw) => raw.1.into(),
        };
        self.represent_as = Some(data);
    }
    /// Apply superscript/subscript conversion after mark_super_subscripts runs.
    /// Must be called after make_ready.
    fn apply_super_subscript(&mut self) {
        if !self.is_superscript && !self.is_subscript {
            return;
        }
        let Some(text) = &self.represent_as else {
            return;
        };
        let converted: String = text
            .chars()
            .map(|c| {
                if self.is_superscript {
                    Self::to_superscript(c).unwrap_or(c)
                } else {
                    Self::to_subscript(c).unwrap_or(c)
                }
            })
            .collect();
        self.represent_as = Some(converted);
    }
    #[inline]
    pub fn get(&self) -> &str {
        self.represent_as.as_ref().expect("make_ready not called")
    }
    /// Convert a single ASCII char to its Unicode superscript equivalent if possible
    fn to_superscript(c: char) -> Option<char> {
        match c {
            '0' => Some('⁰'),
            '1' => Some('¹'),
            '2' => Some('²'),
            '3' => Some('³'),
            '4' => Some('⁴'),
            '5' => Some('⁵'),
            '6' => Some('⁶'),
            '7' => Some('⁷'),
            '8' => Some('⁸'),
            '9' => Some('⁹'),
            '+' => Some('⁺'),
            '-' => Some('⁻'),
            '=' => Some('⁼'),
            '(' => Some('⁽'),
            ')' => Some('⁾'),
            'n' => Some('ⁿ'),
            'i' => Some('ⁱ'),
            'x' => Some('ˣ'),
            _ => None,
        }
    }
    /// Convert a single ASCII char to its Unicode subscript equivalent if possible
    fn to_subscript(c: char) -> Option<char> {
        match c {
            '0' => Some('₀'),
            '1' => Some('₁'),
            '2' => Some('₂'),
            '3' => Some('₃'),
            '4' => Some('₄'),
            '5' => Some('₅'),
            '6' => Some('₆'),
            '7' => Some('₇'),
            '8' => Some('₈'),
            '9' => Some('₉'),
            '+' => Some('₊'),
            '-' => Some('₋'),
            '=' => Some('₌'),
            '(' => Some('₍'),
            ')' => Some('₎'),
            _ => None,
        }
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
        cell_groups: None,
    }
}

#[derive(Debug, Clone)]

pub struct PdfBoxes {
    lines: Vec<PdfBox>,
    /// Cells grouped by table: each inner Vec is one table's cells.
    cell_groups: Option<Vec<Vec<Rect<f32>>>>,
}

#[derive(Debug, Clone)]
pub struct PdfBox {
    id: usize,
    pub rect: Rect<f32>,
}

impl PdfBoxes {
    pub fn get_lines(&self) -> &Vec<PdfBox> {
        &self.lines
    }
    pub fn get_cells(&self) -> Vec<Rect<f32>> {
        self.cell_groups.as_ref().unwrap().iter().flatten().cloned().collect()
    }
    pub fn get_cell_groups(&self) -> &Vec<Vec<Rect<f32>>> {
        self.cell_groups.as_ref().unwrap()
    }
    /// 주어진 lines로 어떤 셀이 만들어졌는지 연산
    /// Grid-based: finds unique row/column positions from horizontal/vertical lines,
    /// then creates cells for each adjacent pair of rows and columns.
    pub fn prepare_cells(&mut self) {
        if self.cell_groups.is_some() {
            return;
        }

        let mut all_groups: Vec<Vec<Rect<f32>>> = Vec::new();

        let all_lines: Vec<Rect<f32>> = self.lines.iter().map(|x| x.rect).collect();
        if all_lines.len() < 4 {
            self.cell_groups = Some(all_groups);
            return;
        }

        // Group lines into table clusters using union-find on rect overlap
        let groups = Self::group_lines_by_overlap(&all_lines);

        for group in groups {
            let h_lines: Vec<&Rect<f32>> = group.iter().filter(|r| r.width() > r.height()).collect();
            let v_lines: Vec<&Rect<f32>> = group.iter().filter(|r| r.height() > r.width()).collect();
            if h_lines.len() < 2 || v_lines.len() < 2 {
                continue;
            }
            let mut table_cells = Vec::new();
            Self::build_grid_cells(&h_lines, &v_lines, &mut table_cells);
            if !table_cells.is_empty() {
                all_groups.push(table_cells);
            }
        }

        self.cell_groups = Some(all_groups);
    }

    /// Group lines into connected components based on rect overlap.
    fn group_lines_by_overlap(lines: &[Rect<f32>]) -> Vec<Vec<Rect<f32>>> {
        let n = lines.len();
        let mut parent: Vec<usize> = (0..n).collect();

        fn find(parent: &mut [usize], i: usize) -> usize {
            if parent[i] != i {
                parent[i] = find(parent, parent[i]);
            }
            parent[i]
        }
        fn union(parent: &mut [usize], a: usize, b: usize) {
            let ra = find(parent, a);
            let rb = find(parent, b);
            if ra != rb {
                parent[ra] = rb;
            }
        }

        let rects_overlap = |a: &Rect<f32>, b: &Rect<f32>| -> bool {
            a.min().x <= b.max().x
                && a.max().x >= b.min().x
                && a.min().y <= b.max().y
                && a.max().y >= b.min().y
        };

        for i in 0..n {
            for j in (i + 1)..n {
                if rects_overlap(&lines[i], &lines[j]) {
                    union(&mut parent, i, j);
                }
            }
        }

        let mut groups: std::collections::HashMap<usize, Vec<Rect<f32>>> =
            std::collections::HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            groups.entry(root).or_default().push(lines[i]);
        }
        groups.into_values().collect()
    }

    /// Build grid cells from a single table's horizontal and vertical lines.
    fn build_grid_cells(
        h_lines: &[&Rect<f32>],
        v_lines: &[&Rect<f32>],
        result: &mut Vec<Rect<f32>>,
    ) {
        let mut row_ys: Vec<f32> = Vec::new();
        for h in h_lines {
            let y = h.center().y;
            if !row_ys.iter().any(|&ry| (ry - y).abs() < 2.0) {
                row_ys.push(y);
            }
        }
        row_ys.sort_by(|a, b| b.partial_cmp(a).unwrap());

        let mut col_xs: Vec<f32> = Vec::new();
        for v in v_lines {
            let x = v.center().x;
            if !col_xs.iter().any(|&cx| (cx - x).abs() < 2.0) {
                col_xs.push(x);
            }
        }
        col_xs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Small epsilon for overlap check to handle PDF coordinate rounding
        let eps = 0.5;
        let rects_overlap = |a: &Rect<f32>, b: &Rect<f32>| -> bool {
            a.min().x <= b.max().x + eps
                && a.max().x >= b.min().x - eps
                && a.min().y <= b.max().y + eps
                && a.max().y >= b.min().y - eps
        };

        for row_pair in row_ys.windows(2) {
            let top_y = row_pair[0];
            let bottom_y = row_pair[1];
            for col_pair in col_xs.windows(2) {
                let left_x = col_pair[0];
                let right_x = col_pair[1];

                let ht: Vec<&&Rect<f32>> = h_lines.iter()
                    .filter(|h| (h.center().y - top_y).abs() < 2.0).collect();
                let hb: Vec<&&Rect<f32>> = h_lines.iter()
                    .filter(|h| (h.center().y - bottom_y).abs() < 2.0).collect();
                let vl: Vec<&&Rect<f32>> = v_lines.iter()
                    .filter(|v| (v.center().x - left_x).abs() < 2.0).collect();
                let vr: Vec<&&Rect<f32>> = v_lines.iter()
                    .filter(|v| (v.center().x - right_x).abs() < 2.0).collect();

                if ht.is_empty() || hb.is_empty() || vl.is_empty() || vr.is_empty() {
                    continue;
                }

                let corners_ok = [
                    ht.iter().any(|h| vl.iter().any(|v| rects_overlap(h, v))),
                    ht.iter().any(|h| vr.iter().any(|v| rects_overlap(h, v))),
                    hb.iter().any(|h| vl.iter().any(|v| rects_overlap(h, v))),
                    hb.iter().any(|h| vr.iter().any(|v| rects_overlap(h, v))),
                ];

                if corners_ok.iter().all(|&ok| ok) {
                    let cell_left = vl.iter().map(|v| v.max().x).fold(f32::MIN, f32::max);
                    let cell_right = vr.iter().map(|v| v.min().x).fold(f32::MAX, f32::min);
                    let cell_top = ht.iter().map(|h| h.min().y).fold(f32::MAX, f32::min);
                    let cell_bottom = hb.iter().map(|h| h.max().y).fold(f32::MIN, f32::max);

                    if cell_left < cell_right && cell_bottom < cell_top {
                        result.push(Rect::new(
                            [cell_left, cell_bottom],
                            [cell_right, cell_top],
                        ));
                    }
                }
            }
        }
    }
    pub fn get_wrapping_cell(&self, rect: &Rect<f32>) -> Option<Rect<f32>> {
        // Use tolerance-based containment since text may extend slightly
        // beyond the inner cell boundaries (past the line edges).
        let tol = 1.0;
        self.cell_groups
            .as_ref()
            .unwrap()
            .iter()
            .flatten()
            .find(|c| {
                rect.min().x >= c.min().x - tol
                    && rect.max().x <= c.max().x + tol
                    && rect.min().y >= c.min().y - tol
                    && rect.max().y <= c.max().y + tol
            })
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
            [5.0, 100.0, 93.0, 3.0], // ends at x=98, gap of 2 to right vertical
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
            [7.0, 50.0, 93.0, 4.0], // starts at x=7, gap of 2 from left vertical
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
            [100.0, 56.0, 6.0, 44.0], // starts at y=56, gap of 2 from bottom horizontal
        ]);
        assert!(cells.is_empty());
    }
    #[test]
    fn test_prepare_cells_y_unfit_2() {
        let cells = generate_cells(&[
            [5.0, 100.0, 95.0, 3.0],
            [5.0, 50.0, 95.0, 4.0],
            [0.0, 54.0, 5.0, 44.0], // ends at y=98, gap of 2 from top horizontal
            [100.0, 54.0, 6.0, 46.0],
        ]);
        assert!(cells.is_empty());
    }
}
