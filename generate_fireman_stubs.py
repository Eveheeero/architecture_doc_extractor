#!/usr/bin/env python3
"""
Generate fireman-compatible instruction_analyze stub files from
result/arm.rs and result/intel.rs Operation pseudocode.

Output: output/intel/{letter}_generated.rs, output/arm/{letter}_generated.rs
"""

import re
import os
import json
import sys
from dataclasses import dataclass
from collections import defaultdict


@dataclass
class Instruction:
    name: str          # enum variant name e.g. "Adc"
    mnemonic: str      # lowercase e.g. "adc"
    operation: str     # Operation pseudocode (or empty)
    first_letter: str  # for file grouping


def parse_rs_enum(filepath: str) -> list[Instruction]:
    """Parse a Rust enum file with doc comments, extract variant names and Operation pseudocode."""
    with open(filepath, "r", encoding="utf-8") as f:
        content = f.read()

    instructions = []
    warnings = []
    lines = content.split("\n")

    # Find enum block boundaries
    in_enum = False
    doc_lines: list[str] = []

    for line in lines:
        # Detect enum start
        if not in_enum:
            if re.match(r"^enum\s+\w+\s*\{", line):
                in_enum = True
            continue

        # Detect enum end
        if line.strip() == "}":
            break

        # Match variant: Name, Name(...), Name { ... }, with optional trailing comma
        variant_match = re.match(
            r"^\s+([A-Z][a-zA-Z0-9]*)(?:\s*[\({].*[\)}])?\s*,\s*$", line
        )
        if variant_match:
            variant_name = variant_match.group(1)
            operation = extract_operation(doc_lines)
            mnemonic = variant_name.lower()

            if doc_lines and not operation:
                # Had doc comments but no Operation section - warn
                has_op_header = any(l.strip() == "## Operation" for l in doc_lines)
                if has_op_header:
                    warnings.append(
                        f"  WARNING: {variant_name} has ## Operation header but code block extraction failed"
                    )

            instructions.append(Instruction(
                name=variant_name,
                mnemonic=mnemonic,
                operation=operation,
                first_letter=mnemonic[0],
            ))
            doc_lines = []
        elif line.strip().startswith("///"):
            stripped = line.strip()
            if stripped.startswith("/// "):
                doc_lines.append(stripped[4:])
            elif stripped == "///":
                doc_lines.append("")
            else:
                doc_lines.append(stripped[3:])
        elif not line.strip():
            pass  # blank line, keep accumulating docs
        else:
            doc_lines = []

    if warnings:
        for w in warnings:
            print(w, file=sys.stderr)

    return instructions


def extract_operation(doc_lines: list[str]) -> str:
    """Extract the Operation pseudocode from doc comment lines.

    Handles two formats:
      Format A: '## Operation' header followed by ```C code fences
      Format B: bare 'Operation' line followed by inline pseudocode (no fences)
    """
    op_start = None
    is_fenced = False

    for i, line in enumerate(doc_lines):
        stripped = line.strip()
        if stripped == "## Operation":
            op_start = i
            is_fenced = True
            break
        if stripped == "Operation":
            op_start = i
            is_fenced = False
            break

    if op_start is None:
        return ""

    if is_fenced:
        return _extract_fenced_operation(doc_lines, op_start)
    else:
        return _extract_bare_operation(doc_lines, op_start)


# Section boundary markers that terminate bare Operation blocks
_SECTION_STOP_PATTERNS = [
    "Intel C/C++ Compiler Intrinsic Equivalent",
    "Flags Affected",
    "SIMD Floating-Point Exceptions",
    "Other Exceptions",
    "Protected Mode Exceptions",
    "Real-Address Mode Exceptions",
    "Virtual-8086 Mode Exceptions",
    "Virtual 8086 Mode Exceptions",
    "Floating-Point Exceptions",
    "Numeric Exceptions",
    "Compatibility Mode Exceptions",
    "64-Bit Mode Exceptions",
]


def _extract_fenced_operation(doc_lines: list[str], op_start: int) -> str:
    """Extract operation from ## Operation + ```C fenced format."""
    code_start = None
    code_end = None
    for i in range(op_start + 1, len(doc_lines)):
        line = doc_lines[i]
        if code_start is None:
            if line.strip().startswith("```"):
                code_start = i + 1
        else:
            if line.strip() == "```":
                code_end = i
                break

    if code_start is None or code_end is None:
        return ""

    code_lines = doc_lines[code_start:code_end]
    return "\n".join(code_lines).strip()


def _extract_bare_operation(doc_lines: list[str], op_start: int) -> str:
    """Extract operation from bare 'Operation' line without code fences.

    Collects lines after 'Operation' until a known section boundary is hit.
    """
    code_lines = []
    for i in range(op_start + 1, len(doc_lines)):
        line = doc_lines[i]
        stripped = line.strip()

        # Stop at known section boundaries
        if any(stripped == pat or stripped.startswith(pat) for pat in _SECTION_STOP_PATTERNS):
            break

        # Stop at markdown section headers
        if stripped.startswith("## "):
            break

        code_lines.append(line)

    # Strip leading/trailing empty lines
    while code_lines and not code_lines[0].strip():
        code_lines.pop(0)
    while code_lines and not code_lines[-1].strip():
        code_lines.pop()

    return "\n".join(code_lines).strip()


def generate_rust_file(instructions: list[Instruction]) -> str:
    """Generate a Rust file with stub functions for the given instructions."""
    parts = [
        "use super::{super::static_register::*, shortcuts::*};",
        "use std::ops::Deref;",
        "",
    ]

    for inst in instructions:
        parts.append("#[box_to_static_reference]")
        parts.append(f"pub(super) fn {inst.mnemonic}() -> &'static [IrStatement] {{")

        if inst.operation:
            parts.append("    // Operation pseudocode:")
            for line in inst.operation.split("\n"):
                parts.append(f"    // {line}")

        parts.append(f'    todo!("implement IR for {inst.mnemonic}")')
        parts.append("}")
        parts.append("")

    return "\n".join(parts)


def clean_generated_files(output_dir: str):
    """Remove previous *_generated.rs files from the output directory."""
    if not os.path.isdir(output_dir):
        return
    for fname in os.listdir(output_dir):
        if fname.endswith("_generated.rs"):
            os.remove(os.path.join(output_dir, fname))


def process_arch(filepath: str, output_dir: str):
    """Process one architecture file and generate stubs."""
    instructions = parse_rs_enum(filepath)

    generated = []
    skipped = []

    with_operation = []
    for inst in instructions:
        if inst.operation:
            with_operation.append(inst)
            generated.append(inst.mnemonic)
        else:
            skipped.append(inst.mnemonic)

    # Group by first letter
    groups: dict[str, list[Instruction]] = defaultdict(list)
    for inst in with_operation:
        groups[inst.first_letter].append(inst)

    # Clean old generated files, then write new ones
    clean_generated_files(output_dir)
    os.makedirs(output_dir, exist_ok=True)
    for letter, group in sorted(groups.items()):
        rust_code = generate_rust_file(group)
        out_path = os.path.join(output_dir, f"{letter}_generated.rs")
        with open(out_path, "w", encoding="utf-8") as f:
            f.write(rust_code)

    return generated, skipped


def main():
    base_dir = os.path.dirname(os.path.abspath(__file__))
    result_dir = os.path.join(base_dir, "result")

    intel_gen, intel_skip = process_arch(
        os.path.join(result_dir, "intel.rs"),
        os.path.join(base_dir, "output", "intel"),
    )

    arm_gen, arm_skip = process_arch(
        os.path.join(result_dir, "arm.rs"),
        os.path.join(base_dir, "output", "arm"),
    )

    print("=== Intel ===")
    print(f"Generated ({len(intel_gen)}):")
    print(json.dumps(intel_gen, indent=2))
    print(f"\nSkipped ({len(intel_skip)}):")
    print(json.dumps(intel_skip, indent=2))

    print("\n=== ARM ===")
    print(f"Generated ({len(arm_gen)}):")
    print(json.dumps(arm_gen, indent=2))
    print(f"\nSkipped ({len(arm_skip)}):")
    print(json.dumps(arm_skip, indent=2))


if __name__ == "__main__":
    main()
