#!/usr/bin/env python3
"""Validate generated MD files in result/intel/."""
import os
import re
import sys

RESULT_DIR = "result/intel"

def validate_filename(name: str) -> list[str]:
    """Check filename is a valid instruction name."""
    errors = []
    if not name:
        errors.append("empty filename")
    elif not name[0].isupper():
        errors.append(f"filename '{name}' doesn't start with uppercase")
    elif re.search(r'[⁻⁰¹²³⁴⁵⁶⁷⁸⁹₀₁₂₃₄₅₆₇₈₉\ufffd]', name):
        errors.append(f"filename '{name}' contains superscript/subscript/replacement chars")
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

    # Check tables have proper separator rows
    in_table = False
    for i, line in enumerate(lines):
        if line.startswith('|') and line.rstrip().endswith('|') and not in_table:
            in_table = True
            # First row of table (header) - next line must be separator
            if i + 1 < len(lines):
                next_line = lines[i+1]
                if not re.match(r'^\|[\s\-:|]+\|$', next_line):
                    errors.append(f"{basename}:{i+1}: table header not followed by separator row")
        elif not (line.startswith('|') and line.rstrip().endswith('|')) and in_table:
            in_table = False

    # Check for replacement characters in content
    if '\ufffd' in content:
        count = content.count('\ufffd')
        errors.append(f"{basename}: contains {count} replacement character(s) U+FFFD")

    # Check for superscript minus used as bullet points (not in mathematical context).
    # Legitimate uses: 2⁻¹², x⁻¹ (preceded by alphanumeric). Illegitimate: line-initial ⁻ as bullet.
    bullet_count = 0
    for line in lines:
        stripped = line.lstrip()
        if stripped.startswith('⁻'):
            bullet_count += 1
    if bullet_count > 0:
        errors.append(f"{basename}: {bullet_count} line(s) start with superscript-minus '⁻' as bullet (should be '-')")

    return errors

def main():
    if not os.path.isdir(RESULT_DIR):
        print(f"ERROR: {RESULT_DIR} not found")
        sys.exit(1)

    md_files = sorted([f for f in os.listdir(RESULT_DIR) if f.endswith('.md')])
    print(f"Found {len(md_files)} MD files")

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

    # Summary
    print(f"\n{'='*60}")
    print(f"VALIDATION SUMMARY")
    print(f"{'='*60}")
    print(f"Total files:        {len(md_files)}")
    print(f"Files with errors:  {files_with_errors}")
    print(f"Filename errors:    {filename_errors}")
    print(f"Content errors:     {content_errors}")
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
