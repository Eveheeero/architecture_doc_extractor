#!/usr/bin/env python3
"""Validate generated ARM MD files in result/arm/."""
import os
import re
import subprocess
import sys

RESULT_DIR = "result/arm"
ENUM_FILE = "result/arm.rs"


def validate_filename(name: str) -> list[str]:
    """Check filename follows heading__id convention."""
    errors = []
    if not name:
        errors.append("empty filename")
        return errors
    if not name[0].isupper():
        errors.append(f"filename '{name}' doesn't start with uppercase")
    if "__" not in name:
        errors.append(f"filename '{name}' missing '__' separator (heading__id)")
    if re.search(r'[\ufffd]', name):
        errors.append(f"filename '{name}' contains replacement char U+FFFD")
    return errors


def validate_content(filepath: str) -> list[str]:
    """Check MD file content is properly formatted."""
    errors = []
    with open(filepath, 'r') as f:
        content = f.read()
    lines = content.split('\n')
    basename = os.path.basename(filepath).replace('.md', '')

    # Must start with H1
    if not lines or not lines[0].startswith('# '):
        errors.append(f"{basename}: missing H1 title")
        return errors

    title = lines[0][2:].strip()
    if not title:
        errors.append(f"{basename}: empty H1 title")

    # Must have at least some content beyond the title
    non_empty = [l for l in lines[1:] if l.strip()]
    if len(non_empty) < 2:
        errors.append(f"{basename}: too little content ({len(non_empty)} non-empty lines)")

    # Check for H2 sections
    h2_sections = [l for l in lines if l.startswith('## ')]
    if not h2_sections:
        errors.append(f"{basename}: no H2 sections found")

    # Check for expected sections
    section_names = set(s[3:].strip() for s in h2_sections)
    # At least Encoding or Operands or Description should exist
    has_structure = bool(section_names & {"Encoding", "Operands", "Description", "Decode", "Operation"})
    if not has_structure:
        errors.append(f"{basename}: missing standard sections (has: {section_names})")

    # Check ASM template formatting (should have spaces around operands)
    for i, line in enumerate(lines):
        if line.startswith('- `') and line.endswith('`'):
            # ASM template line
            asm = line[3:-1]
            # Check for missing spaces: consecutive ><
            if '><' in asm:
                errors.append(f"{basename}:{i+1}: ASM template missing spaces: '{asm[:60]}...'")

    # Check tables have proper separator rows
    in_table = False
    for i, line in enumerate(lines):
        if line.startswith('|') and line.rstrip().endswith('|') and not in_table:
            in_table = True
            if i + 1 < len(lines):
                next_line = lines[i+1]
                if not re.match(r'^\|[\s\-:|]+\|$', next_line):
                    errors.append(f"{basename}:{i+1}: table header not followed by separator row")
        elif not (line.startswith('|') and line.rstrip().endswith('|')) and in_table:
            in_table = False

    # Check pseudocode blocks are properly fenced
    in_code = False
    for i, line in enumerate(lines):
        if line.strip() == '```':
            in_code = not in_code
    if in_code:
        errors.append(f"{basename}: unclosed code fence")

    # Check for missing whitespace in pseudocode (common issue: "thenSP[]" or "elseX[")
    in_code = False
    for i, line in enumerate(lines):
        if line.strip() == '```':
            in_code = not in_code
            continue
        if in_code:
            # Check for "then" or "else" followed by identifier without space
            if re.search(r'\bthen[A-Z]', line):
                errors.append(f"{basename}:{i+1}: pseudocode missing space after 'then': '{line.strip()[:60]}'")
            if re.search(r'\belse[A-Z]', line):
                errors.append(f"{basename}:{i+1}: pseudocode missing space after 'else': '{line.strip()[:60]}'")

    # Check for replacement characters in content
    if '\ufffd' in content:
        count = content.count('\ufffd')
        errors.append(f"{basename}: contains {count} replacement character(s) U+FFFD")

    # Check operand section integrity: no bare lines (all must start with "- " or be empty)
    in_operands = False
    for i, line in enumerate(lines):
        if line == '## Operands':
            in_operands = True
            continue
        if line.startswith('## ') and in_operands:
            in_operands = False
        if in_operands and line.strip() and not line.startswith('- ') and not line == '':
            errors.append(f"{basename}:{i+1}: broken operand line (not a bullet): '{line.strip()[:60]}'")

    return errors


def validate_enum() -> list[str]:
    """Validate the generated Rust enum file."""
    errors = []
    if not os.path.exists(ENUM_FILE):
        errors.append(f"{ENUM_FILE} not found")
        return errors

    with open(ENUM_FILE, 'r') as f:
        content = f.read()
    lines = content.split('\n')

    if not lines or not lines[0].startswith('enum '):
        errors.append(f"{ENUM_FILE}: doesn't start with 'enum' declaration")
        return errors

    # Check for valid Rust identifiers in enum variants
    variant_count = 0
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith('///') or stripped == '' or stripped.startswith('enum') or stripped == '}':
            continue
        # Should be a variant line like "    Abc,"
        if stripped.endswith(','):
            variant_name = stripped[:-1].strip()
            variant_count += 1
            if not re.match(r'^[A-Z][A-Za-z0-9_]*$', variant_name):
                errors.append(f"{ENUM_FILE}:{i+1}: invalid variant name '{variant_name}'")
        elif stripped:
            # Unexpected line
            pass

    if variant_count == 0:
        errors.append(f"{ENUM_FILE}: no enum variants found")
    else:
        print(f"  Enum variants: {variant_count}")

    # Check doc comments have proper H1
    doc_h1_count = 0
    for line in lines:
        if line.strip().startswith('/// #'):
            doc_h1_count += 1
    if doc_h1_count == 0:
        errors.append(f"{ENUM_FILE}: no doc H1 headings found")

    # Check for Unknown variant (empty mnemonic)
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped in ("Unknown,", "Unknown"):
            errors.append(f"{ENUM_FILE}:{i+1}: 'Unknown' variant found (empty mnemonic)")

    # Check lines outside doc comments (escaped content)
    for i, line in enumerate(lines):
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith('///') or stripped.startswith('enum') or stripped == '}':
            continue
        if stripped.endswith(',') and re.match(r'^[A-Z][A-Za-z0-9_]*,$', stripped):
            continue
        # Unexpected bare line — likely escaped doc content
        errors.append(f"{ENUM_FILE}:{i+1}: unexpected line outside doc comment: '{stripped[:60]}'")

    # Try compiling with rustc
    try:
        result = subprocess.run(
            ["rustc", "--edition", "2021", "--crate-type", "lib", ENUM_FILE],
            capture_output=True, text=True, timeout=30
        )
        if result.returncode != 0:
            # Filter out dead_code warning
            real_errors = [l for l in result.stderr.split('\n')
                          if l.startswith('error') and 'dead_code' not in l]
            if real_errors:
                errors.append(f"{ENUM_FILE}: rustc compilation failed with {len(real_errors)} error(s)")
                for e in real_errors[:5]:
                    errors.append(f"  {e}")
        else:
            print("  rustc compilation: OK")
            # Clean up .rlib
            for f in os.listdir('.'):
                if f.startswith('libarm') and f.endswith('.rlib'):
                    os.remove(f)
    except FileNotFoundError:
        print("  rustc not found, skipping compilation check")
    except subprocess.TimeoutExpired:
        errors.append(f"{ENUM_FILE}: rustc compilation timed out")

    return errors


def main():
    if not os.path.isdir(RESULT_DIR):
        print(f"ERROR: {RESULT_DIR} not found")
        sys.exit(1)

    md_files = sorted([f for f in os.listdir(RESULT_DIR) if f.endswith('.md')])
    print(f"Found {len(md_files)} ARM MD files")

    all_errors = []
    filename_errors = 0
    content_errors = 0
    files_with_errors = 0

    for f in md_files:
        name = f.replace('.md', '')
        file_errors = []

        # Validate filename
        ferrs = validate_filename(name)
        file_errors.extend(ferrs)
        filename_errors += len(ferrs)

        # Validate content
        cerrs = validate_content(os.path.join(RESULT_DIR, f))
        file_errors.extend(cerrs)
        content_errors += len(cerrs)

        if file_errors:
            files_with_errors += 1
            all_errors.extend(file_errors)

    # Validate enum
    print(f"\nValidating {ENUM_FILE}...")
    enum_errors = validate_enum()
    all_errors.extend(enum_errors)

    # Summary
    print(f"\n{'='*60}")
    print(f"ARM VALIDATION SUMMARY")
    print(f"{'='*60}")
    print(f"Total MD files:     {len(md_files)}")
    print(f"Files with errors:  {files_with_errors}")
    print(f"Filename errors:    {filename_errors}")
    print(f"Content errors:     {content_errors}")
    print(f"Enum errors:        {len(enum_errors)}")
    print(f"{'='*60}")

    if all_errors:
        print(f"\nErrors (showing first 50):")
        for err in all_errors[:50]:
            print(f"  - {err}")
        if len(all_errors) > 50:
            print(f"  ... and {len(all_errors) - 50} more")
        sys.exit(1)
    else:
        print("\nAll files passed validation!")
        sys.exit(0)


if __name__ == '__main__':
    main()
