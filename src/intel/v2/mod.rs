use crate::intel::result::{Instruction, MdTable};
use crate::pdf::v2::*;
use geo::Rect;
use std::collections::BTreeMap;
use tracing::debug;

/// font_scale tolerance-band classification
#[derive(Debug, Clone, Copy, PartialEq)]
enum ScaleClass {
    /// ~12: instruction title (h1)
    Title,
    /// ~9.96: section heading (h2)
    Heading,
    /// ~9: body text
    Body,
    /// ~7.98: header/footer
    SmallText,
}

fn classify_font_scale(scale: f32) -> ScaleClass {
    if scale > 11.0 {
        ScaleClass::Title
    } else if scale > 9.5 {
        ScaleClass::Heading
    } else if scale > 8.5 {
        ScaleClass::Body
    } else {
        ScaleClass::SmallText
    }
}

/// Current section being parsed
#[derive(Debug, Clone, PartialEq)]
enum CurrentSection {
    None,
    InstructionTable,
    Description,
    Operation,
    FlagsAffected,
    CppIntrinsic,
    Exceptions(String),
    /// Any heading not matching known sections, stored with its display name
    Other(String),
}

/// Check if a title-scale text is actually an instruction title.
/// Instruction titles start with uppercase ASCII letters (e.g. "AAA", "ADDPD",
/// "MOVDQA,VMOVDQA32/64") followed by a separator and summary.
fn is_instruction_title(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Must start with an uppercase ASCII letter
    let first = trimmed.chars().next().unwrap();
    if !first.is_ascii_uppercase() {
        return false;
    }
    // Extract the instruction name part (before any separator)
    let name_part = if let Some(idx) =
        trimmed.find(|c: char| c == '\u{2014}' || c == '\u{2013}' || c == '\u{97}')
    {
        &trimmed[..idx]
    } else if let Some(idx) = trimmed.find(" - ") {
        &trimmed[..idx]
    } else if let Some(idx) = trimmed.find('-') {
        &trimmed[..idx]
    } else {
        trimmed
    };
    let name_part = name_part.trim();
    // The instruction name must be primarily uppercase letters, digits, and allowed separators
    // like / , [ ] and spaces
    let valid_chars = name_part.chars().all(|c| {
        c.is_ascii_uppercase()
            || c.is_ascii_digit()
            || matches!(c, '/' | ',' | '[' | ']' | ' ' | '.')
    });
    if !valid_chars {
        return false;
    }
    // Must contain at least 2 consecutive uppercase letters
    let has_instruction = name_part
        .as_bytes()
        .windows(2)
        .any(|w| w[0].is_ascii_uppercase() && w[1].is_ascii_uppercase());
    has_instruction
}

/// Parse title line like "AAA—ASCII Adjust After Addition"
/// into (instruction_name, summary)
fn parse_title(text: &str) -> (String, String) {
    // Try common separators: em-dash, en-dash, hyphen surrounded by spaces
    for sep in ["\u{2014}", "\u{2013}", " - ", "\u{97}"] {
        if let Some((name, summary)) = text.split_once(sep) {
            let name = name.trim().to_owned();
            let summary = summary.trim().to_owned();
            if !name.is_empty() && !summary.is_empty() {
                return (name, summary);
            }
        }
    }
    // Fallback: first hyphen
    if let Some((name, summary)) = text.split_once('-') {
        return (name.trim().to_owned(), summary.trim().to_owned());
    }
    (text.trim().to_owned(), String::new())
}

/// Group strings that fall inside cells into structured tables.
/// Returns (tables with top-Y position, indices of consumed strings).
/// Each table group from PdfBoxes produces a separate MdTable.
fn build_tables_from_cells(
    strings: &[PdfString],
    boxes: &PdfBoxes,
) -> (Vec<(f32, MdTable)>, Vec<usize>) {
    let mut tables = Vec::new();
    let mut consumed_indices = Vec::new();

    let cell_groups = boxes.get_cell_groups();
    if cell_groups.is_empty() {
        return (tables, consumed_indices);
    }

    for group_cells in cell_groups {
        if group_cells.is_empty() {
            continue;
        }

        // Collect unique Y/X boundaries for this table group
        let mut y_bounds: Vec<f32> = Vec::new();
        let mut x_bounds: Vec<f32> = Vec::new();
        for cell in group_cells {
            push_unique_sorted(&mut y_bounds, cell.min().y, 1.0);
            push_unique_sorted(&mut y_bounds, cell.max().y, 1.0);
            push_unique_sorted(&mut x_bounds, cell.min().x, 1.0);
            push_unique_sorted(&mut x_bounds, cell.max().x, 1.0);
        }
        y_bounds.sort_by(|a, b| b.partial_cmp(a).unwrap());
        x_bounds.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Map characters to cells in this group, splitting strings at cell boundaries
        let mut cell_contents: BTreeMap<usize, Vec<(f32, f32, String)>> = BTreeMap::new();
        let mut group_consumed = Vec::new();
        let tol = 1.0;

        for (idx, s) in strings.iter().enumerate() {
            if consumed_indices.contains(&idx) {
                continue;
            }
            // Check each character's center against cells in this group
            let mut char_cells: BTreeMap<usize, String> = BTreeMap::new();
            let mut any_in_group = false;
            for ch in s.chars() {
                let cx = ch.rect.center().x;
                let cy = ch.rect.center().y;
                if let Some((ci, _)) = group_cells.iter().enumerate().find(|(_, c)| {
                    cx >= c.min().x - tol
                        && cx <= c.max().x + tol
                        && cy >= c.min().y - tol
                        && cy <= c.max().y + tol
                }) {
                    any_in_group = true;
                    char_cells.entry(ci).or_default().push_str(ch.get());
                }
            }
            if any_in_group {
                group_consumed.push(idx);
                for (ci, text) in char_cells {
                    let cell = &group_cells[ci];
                    cell_contents.entry(ci).or_default().push((
                        cell.center().x,
                        cell.center().y,
                        text,
                    ));
                }
            }
        }

        if group_consumed.is_empty() {
            continue;
        }
        consumed_indices.extend(&group_consumed);

        // Determine row/col for each cell
        let mut grid: BTreeMap<(usize, usize), String> = BTreeMap::new();
        let mut max_row = 0usize;
        let mut max_col = 0usize;

        for (cell_idx, texts) in &cell_contents {
            let cell = &group_cells[*cell_idx];
            let row = find_band_index(&y_bounds, cell.center().y);
            let col = find_band_index_asc(&x_bounds, cell.center().x);
            let combined: String = texts
                .iter()
                .map(|(_, _, t)| t.trim().to_owned())
                .collect::<Vec<_>>()
                .join(" ");
            grid.insert((row, col), combined);
            if row > max_row {
                max_row = row;
            }
            if col > max_col {
                max_col = col;
            }
        }

        // Build MdTable: first row = headers
        let num_cols = max_col + 1;
        let mut headers = Vec::new();
        for c in 0..num_cols {
            headers.push(grid.get(&(0, c)).cloned().unwrap_or_default());
        }
        let mut rows = Vec::new();
        for r in 1..=max_row {
            let mut row = Vec::new();
            for c in 0..num_cols {
                row.push(grid.get(&(r, c)).cloned().unwrap_or_default());
            }
            rows.push(row);
        }

        if !headers.is_empty() {
            let top_y = group_cells
                .iter()
                .map(|c| c.max().y)
                .fold(f32::MIN, f32::max);
            tables.push((top_y, MdTable { headers, rows }));
        }
    }

    (tables, consumed_indices)
}

fn push_unique_sorted(v: &mut Vec<f32>, val: f32, tolerance: f32) {
    if !v.iter().any(|&x| (x - val).abs() < tolerance) {
        v.push(val);
    }
}

/// Find which band a Y value falls into (descending order - top down)
fn find_band_index(bounds: &[f32], y: f32) -> usize {
    for (i, window) in bounds.windows(2).enumerate() {
        if y <= window[0] && y >= window[1] {
            return i;
        }
    }
    bounds.len().saturating_sub(1)
}

/// Find which band an X value falls into (ascending order - left to right)
fn find_band_index_asc(bounds: &[f32], x: f32) -> usize {
    for (i, window) in bounds.windows(2).enumerate() {
        if x >= window[0] && x <= window[1] {
            return i;
        }
    }
    bounds.len().saturating_sub(1)
}

/// Calculate indentation for operation lines based on X position.
/// Clusters X positions into bands (±3pt tolerance), then assigns
/// indent levels based on band index rather than raw distance.
fn indent_operation_lines(lines: &[(f32, String)]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    // Collect unique X positions and cluster into bands (±3pt tolerance)
    let mut x_positions: Vec<f32> = lines.iter().map(|(x, _)| *x).collect();
    x_positions.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut bands: Vec<f32> = Vec::new();
    for &x in &x_positions {
        if !bands.iter().any(|&b| (b - x).abs() < 3.0) {
            bands.push(x);
        }
    }
    bands.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut result = Vec::new();
    for (x, text) in lines {
        // Find the band index for this X position
        let indent_level = bands.iter().position(|&b| (b - x).abs() < 3.0).unwrap_or(0);
        let spaces = "    ".repeat(indent_level);
        result.push(format!("{spaces}{text}"));
    }
    result.join("\n")
}

/// Check if text is a header/footer line (skip these)
fn is_header_footer(s: &PdfString, page_y_range: (f32, f32)) -> bool {
    let scale = classify_font_scale(s.font_scale());
    if scale == ScaleClass::SmallText {
        return true;
    }
    // Also skip by Y position: top 5% and bottom 5% of page
    let rect = s.rect();
    let page_height = page_y_range.1 - page_y_range.0;
    if page_height > 0.0 {
        let y_center = rect.center().y;
        if y_center > page_y_range.1 - page_height * 0.05
            || y_center < page_y_range.0 + page_height * 0.05
        {
            return true;
        }
    }
    false
}

/// Detect a section heading from text content.
/// Returns the matching section, or `Other(heading_text)` for unrecognized headings.
fn detect_section(text: &str) -> CurrentSection {
    let trimmed = text.trim();
    if trimmed.starts_with("Opcode")
        || trimmed == "Instruction"
        || trimmed == "Instruction Operand Encoding"
    {
        return CurrentSection::InstructionTable;
    }
    if trimmed == "Description" {
        return CurrentSection::Description;
    }
    if trimmed == "Operation" {
        return CurrentSection::Operation;
    }
    if trimmed == "Flags Affected" || trimmed == "FPU Flags Affected" {
        return CurrentSection::FlagsAffected;
    }
    if trimmed.contains("Intrinsic") {
        return CurrentSection::CppIntrinsic;
    }
    if trimmed.ends_with("Exceptions") {
        return CurrentSection::Exceptions(trimmed.to_owned());
    }
    // Table labels and other known sub-section headings
    if trimmed.starts_with("Table ") || trimmed == "Effective Operand Size" || trimmed == "NOTES" {
        return CurrentSection::Other(trimmed.to_owned());
    }
    CurrentSection::None
}

/// Extract (instruction_mnemonic, description) pairs from an Instruction table.
/// Handles two column layouts:
/// 1. Separate "Opcode" and "Instruction" columns (e.g., AAA, PUSH)
/// 2. Merged "Opcode/ Instruction" column (e.g., ADDPD, MOV)
fn extract_instruction_variants(table: &MdTable, out: &mut Vec<(String, String)>) {
    if table.headers.is_empty() || table.rows.is_empty() {
        return;
    }

    // Find the Description column index
    let desc_idx = table.headers.iter().position(|h| h.trim() == "Description");
    let desc_idx = match desc_idx {
        Some(i) => i,
        None => return,
    };

    // Find the Instruction column: prefer separate "Instruction" column,
    // fall back to merged "Opcode/ Instruction" or "Opcode/Instruction"
    let instr_idx = table
        .headers
        .iter()
        .position(|h| {
            let t = h.trim();
            t == "Instruction" || t == "1 Instruction"
        })
        .or_else(|| {
            table.headers.iter().position(|h| {
                let t = h.trim().to_lowercase();
                t.contains("instruction") && !t.contains("operand")
            })
        });
    let instr_idx = match instr_idx {
        Some(i) => i,
        None => return,
    };

    let is_merged = {
        let h = table.headers[instr_idx].trim().to_lowercase();
        h.contains("opcode")
    };

    for row in &table.rows {
        let instr_cell = row.get(instr_idx).map(|s| s.trim()).unwrap_or("");
        let desc_cell = row.get(desc_idx).map(|s| s.trim()).unwrap_or("");
        if instr_cell.is_empty() || desc_cell.is_empty() {
            continue;
        }

        let mnemonic = if is_merged {
            // Extract instruction mnemonic from merged "opcode instruction operands"
            // e.g., "66 0F 58 /r ADDPD xmm1, xmm2/m128"
            extract_mnemonic_from_merged(instr_cell)
        } else {
            instr_cell.to_owned()
        };

        if !mnemonic.is_empty() {
            out.push((mnemonic, desc_cell.to_owned()));
        }
    }
}

/// Extract the instruction mnemonic + operands from a merged "Opcode/Instruction" cell.
/// Input: "66 0F 58 /r ADDPD xmm1, xmm2/m128"
/// Output: "ADDPD xmm1, xmm2/m128"
///
/// Strategy: find the first token that starts with an uppercase letter and is at least
/// 2 chars long and not a hex byte (like "0F"). Everything from that token onward is
/// the instruction mnemonic with operands.
fn extract_mnemonic_from_merged(cell: &str) -> String {
    let tokens: Vec<&str> = cell.split_whitespace().collect();
    for (i, token) in tokens.iter().enumerate() {
        // Skip known opcode prefixes: hex bytes (0F, 66, F2, etc.), /r, /0-/7, ib, iw, id, etc.
        if token.starts_with('/') {
            continue;
        }
        if token.len() <= 2
            && token
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c.is_ascii_uppercase())
        {
            continue;
        }
        // Check for opcode-like patterns: "REX.W", "VEX.*", "EVEX.*", "NP"
        if token.starts_with("REX") || token.starts_with("VEX") || token.starts_with("EVEX") {
            continue;
        }
        if *token == "NP"
            || *token == "+rb"
            || *token == "+rw"
            || *token == "+rd"
            || *token == "+ro"
        {
            continue;
        }
        // Skip common immediate size markers
        if matches!(
            *token,
            "ib" | "iw" | "id" | "io" | "cb" | "cw" | "cd" | "cp" | "ct"
        ) {
            continue;
        }
        // This token looks like a mnemonic
        if token
            .chars()
            .next()
            .map_or(false, |c| c.is_ascii_uppercase())
        {
            return tokens[i..].join(" ");
        }
    }
    // Fallback: return the whole cell
    cell.to_owned()
}

pub(crate) fn parse_instructions(mut d: Vec<(Vec<PdfString>, PdfBoxes)>) -> Vec<Instruction> {
    let mut result: Vec<Instruction> = Vec::new();
    let mut current = Instruction::default();
    let mut section = CurrentSection::None;
    let mut has_current = false;
    let mut operation_lines: Vec<(f32, String)> = Vec::new();

    // Collect table section name for table association
    let mut table_section_name = String::from("Instruction");

    for (sorted_strings, boxes) in &mut d {
        boxes.prepare_cells();

        if sorted_strings.is_empty() {
            continue;
        }

        // Determine page Y range for header/footer detection
        let page_y_range = {
            let mut min_y = f32::MAX;
            let mut max_y = f32::MIN;
            for s in sorted_strings.iter() {
                let r = s.rect();
                if r.min().y < min_y {
                    min_y = r.min().y;
                }
                if r.max().y > max_y {
                    max_y = r.max().y;
                }
            }
            (min_y, max_y)
        };

        // Build tables from cells on this page (one per table group)
        let (mut page_tables, consumed_indices) = build_tables_from_cells(sorted_strings, boxes);
        // Sort tables top-down (higher Y = higher on page = first)
        page_tables.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let has_tables = !page_tables.is_empty();

        // For each table, find the closest heading above it using the table's top Y
        let mut table_sections: Vec<(String, MdTable)> = Vec::new();
        for (table_top_y, table) in page_tables {
            let mut section_name = table_section_name.clone();
            let mut best_y_dist = f32::MAX;

            for s in sorted_strings.iter() {
                if is_header_footer(s, page_y_range) {
                    continue;
                }
                let scale = classify_font_scale(s.font_scale());
                let sy = s.rect().center().y;
                // Only headings above the table
                if sy <= table_top_y {
                    continue;
                }
                let dist = sy - table_top_y;
                if scale == ScaleClass::Title && dist < best_y_dist {
                    section_name = "Instruction".to_owned();
                    best_y_dist = dist;
                } else if scale == ScaleClass::Heading && dist < best_y_dist {
                    let detected = detect_section(&s.get());
                    if detected == CurrentSection::InstructionTable {
                        section_name = "Instruction".to_owned();
                    } else {
                        // Use heading text as section name (e.g. "Instruction Operand Encoding")
                        section_name = s.get().trim().to_owned();
                    }
                    best_y_dist = dist;
                }
            }

            table_sections.push((section_name, table));
        }

        // Operation lines are accumulated across pages in the outer scope

        for (idx, s) in sorted_strings.iter().enumerate() {
            // Skip header/footer
            if is_header_footer(s, page_y_range) {
                continue;
            }

            // Skip strings already consumed by table building
            if consumed_indices.contains(&idx) {
                continue;
            }

            let text = s.get();
            let scale = classify_font_scale(s.font_scale());

            match scale {
                ScaleClass::Title => {
                    if !is_instruction_title(&text) {
                        // Title-scale text that isn't an instruction name:
                        // Check if it's a section heading rendered at title scale
                        if has_current && !current.title.is_empty() {
                            let new_section = detect_section(text.trim());
                            if new_section != CurrentSection::None {
                                if section == CurrentSection::Operation
                                    && !operation_lines.is_empty()
                                {
                                    current.operation = indent_operation_lines(&operation_lines);
                                    operation_lines.clear();
                                }
                                section = new_section;
                                table_section_name = text.trim().to_owned();
                            } else {
                                match &section {
                                    CurrentSection::Description => {
                                        current.description.push(text.trim().to_owned());
                                    }
                                    CurrentSection::Operation => {
                                        operation_lines.push((s.x(), text.trim().to_owned()));
                                    }
                                    CurrentSection::FlagsAffected => {
                                        if current.flag_affected.is_empty() {
                                            current.flag_affected = text.trim().to_owned();
                                        } else {
                                            current.flag_affected.push(' ');
                                            current.flag_affected.push_str(text.trim());
                                        }
                                    }
                                    CurrentSection::CppIntrinsic => {
                                        current.c_and_cpp_equivalent.push(text.trim().to_owned());
                                    }
                                    CurrentSection::Exceptions(kind) => {
                                        current
                                            .exceptions
                                            .entry(kind.clone())
                                            .or_default()
                                            .push(text.trim().to_owned());
                                    }
                                    CurrentSection::Other(name) => {
                                        if let Some(entry) = current
                                            .other_sections
                                            .iter_mut()
                                            .find(|(n, _)| n == name)
                                        {
                                            entry.1.push(text.trim().to_owned());
                                        } else {
                                            current
                                                .other_sections
                                                .push((name.clone(), vec![text.trim().to_owned()]));
                                        }
                                    }
                                    _ => {
                                        current.description.push(text.trim().to_owned());
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Flush operation lines before starting new instruction
                    if !operation_lines.is_empty() {
                        current.operation = indent_operation_lines(&operation_lines);
                        operation_lines.clear();
                    }

                    // New instruction starts
                    if has_current && !current.title.is_empty() {
                        result.push(current);
                        current = Instruction::default();
                    }
                    let (name, summary) = parse_title(&text);
                    current.title = name;
                    current.summary = summary;
                    has_current = true;
                    section = CurrentSection::None;
                    table_section_name = String::from("Instruction");
                }
                ScaleClass::Heading => {
                    let new_section = detect_section(&text);
                    if new_section != CurrentSection::None {
                        // Known section heading — flush and switch
                        if section == CurrentSection::Operation && !operation_lines.is_empty() {
                            current.operation = indent_operation_lines(&operation_lines);
                            operation_lines.clear();
                        }
                        section = new_section;
                        table_section_name = text.trim().to_owned();
                    } else {
                        // Not a known heading — treat as body content
                        // in the current section
                        match &section {
                            CurrentSection::Description => {
                                current.description.push(text.trim().to_owned());
                            }
                            CurrentSection::Operation => {
                                operation_lines.push((s.x(), text.trim().to_owned()));
                            }
                            CurrentSection::FlagsAffected => {
                                if current.flag_affected.is_empty() {
                                    current.flag_affected = text.trim().to_owned();
                                } else {
                                    current.flag_affected.push(' ');
                                    current.flag_affected.push_str(text.trim());
                                }
                            }
                            CurrentSection::CppIntrinsic => {
                                current.c_and_cpp_equivalent.push(text.trim().to_owned());
                            }
                            CurrentSection::Exceptions(kind) => {
                                current
                                    .exceptions
                                    .entry(kind.clone())
                                    .or_default()
                                    .push(text.trim().to_owned());
                            }
                            CurrentSection::Other(name) => {
                                if let Some(entry) =
                                    current.other_sections.iter_mut().find(|(n, _)| n == name)
                                {
                                    entry.1.push(text.trim().to_owned());
                                } else {
                                    current
                                        .other_sections
                                        .push((name.clone(), vec![text.trim().to_owned()]));
                                }
                            }
                            _ => {
                                // No active section yet — default to description
                                current.description.push(text.trim().to_owned());
                            }
                        }
                    }
                }
                ScaleClass::Body => {
                    // Some PDFs render section headings at body font scale.
                    // Detect known section keywords and treat them as headings.
                    let new_section = detect_section(text.trim());
                    if new_section != CurrentSection::None {
                        // Flush operation lines when leaving Operation section
                        if section == CurrentSection::Operation && !operation_lines.is_empty() {
                            current.operation = indent_operation_lines(&operation_lines);
                            operation_lines.clear();
                        }
                        section = new_section;
                        table_section_name = text.trim().to_owned();
                    } else {
                        match &section {
                            CurrentSection::Description => {
                                current.description.push(text.trim().to_owned());
                            }
                            CurrentSection::Operation => {
                                operation_lines.push((s.x(), text.trim().to_owned()));
                            }
                            CurrentSection::FlagsAffected => {
                                if current.flag_affected.is_empty() {
                                    current.flag_affected = text.trim().to_owned();
                                } else {
                                    current.flag_affected.push(' ');
                                    current.flag_affected.push_str(text.trim());
                                }
                            }
                            CurrentSection::CppIntrinsic => {
                                current.c_and_cpp_equivalent.push(text.trim().to_owned());
                            }
                            CurrentSection::Exceptions(kind) => {
                                current
                                    .exceptions
                                    .entry(kind.clone())
                                    .or_default()
                                    .push(text.trim().to_owned());
                            }
                            CurrentSection::Other(name) => {
                                // Find or create the named section
                                if let Some(entry) =
                                    current.other_sections.iter_mut().find(|(n, _)| n == name)
                                {
                                    entry.1.push(text.trim().to_owned());
                                } else {
                                    current
                                        .other_sections
                                        .push((name.clone(), vec![text.trim().to_owned()]));
                                }
                            }
                            _ => {
                                // No active section yet — default to description
                                current.description.push(text.trim().to_owned());
                            }
                        }
                    }
                }
                ScaleClass::SmallText => {
                    // Already filtered by is_header_footer, but just in case
                }
            }
        }

        // Associate tables with current instruction and extract per-variant descriptions
        if has_tables && has_current {
            for (name, table) in table_sections {
                if name == "Instruction" && current.instructions.is_empty() {
                    extract_instruction_variants(&table, &mut current.instructions);
                }
                current.tables.push((name, table));
            }
        }
    }

    // Flush remaining operation lines
    if !operation_lines.is_empty() {
        current.operation = indent_operation_lines(&operation_lines);
    }

    // Flush last instruction
    if has_current && !current.title.is_empty() {
        result.push(current);
    }

    debug!("Parsed {} instructions", result.len());
    result
}
