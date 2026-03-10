#!/usr/bin/env python3
"""
Validate generated IR instruction bodies against pseudocode semantics.

Parses each ?_generated.rs file, extracts function names, pseudocode doc comments,
and IR bodies. For non-exception functions, applies semantic checks using the
fireman IR API patterns as ground truth.

Runs validation per-file in parallel using ThreadPoolExecutor.
"""

import re
import os
import sys
from dataclasses import dataclass, field
from concurrent.futures import ThreadPoolExecutor, as_completed
from glob import glob


@dataclass
class FunctionIR:
    name: str
    pseudocode: str
    body: str
    is_exception: bool
    file_path: str
    arch: str


@dataclass
class ValidationResult:
    name: str
    verdict: str  # CORRECT / ACCEPTABLE / WRONG / EXCEPTION_STUB
    issues: list[str] = field(default_factory=list)
    arch: str = ''


# ============================================================
# Parser: Extract functions from generated .rs files
# ============================================================

def parse_generated_rs(filepath: str, arch: str) -> list[FunctionIR]:
    """Parse a generated .rs file and extract all function definitions."""
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    functions: list[FunctionIR] = []
    lines = content.split('\n')
    i = 0
    n = len(lines)
    # Accumulated pseudocode — persists until consumed by next function
    pending_pseudocode: list[str] = []

    while i < n:
        # Scan for /// # Pseudocode
        if lines[i].strip().startswith('/// # Pseudocode'):
            pending_pseudocode = []
            i += 1
            # Skip /// ```text
            if i < n and '```text' in lines[i]:
                i += 1
            # Collect until /// ```
            while i < n and lines[i].strip() != '/// ```':
                line = lines[i].strip()
                if line.startswith('/// '):
                    pending_pseudocode.append(line[4:])
                elif line == '///':
                    pending_pseudocode.append('')
                i += 1
            if i < n:
                i += 1  # skip closing ```
            continue

        # Look for function signature
        fn_match = re.match(r'^pub\(super\)\s+fn\s+(\w+)\(\)\s*->', lines[i])
        if fn_match:
            fname = fn_match.group(1)
            # Collect body until closing }
            body_lines: list[str] = []
            i += 1
            brace_depth = 1
            while i < n and brace_depth > 0:
                line = lines[i]
                brace_depth += line.count('{') - line.count('}')
                if brace_depth > 0:
                    body_lines.append(line)
                i += 1

            body = '\n'.join(body_lines).strip()
            pseudocode = '\n'.join(pending_pseudocode).strip()
            pending_pseudocode = []  # consumed
            is_exception = bool(re.match(r'^\s*\[exception\(".*?"\)\]\.into\(\)\s*$', body))

            functions.append(FunctionIR(
                name=fname,
                pseudocode=pseudocode,
                body=body,
                is_exception=is_exception,
                file_path=filepath,
                arch=arch,
            ))
            pseudocode_lines = []
            continue

        # #[box_to_static_reference] — skip
        if lines[i].strip() == '#[box_to_static_reference]':
            i += 1
            continue

        i += 1

    return functions


# ============================================================
# Semantic Checks
# ============================================================

def _body_contains(body: str, *patterns: str) -> bool:
    """Check if body contains all given substring patterns."""
    return all(p in body for p in patterns)


def _body_has_any(body: str, *patterns: str) -> bool:
    """Check if body contains any of the given patterns."""
    return any(p in body for p in patterns)


def _check_flags_before_assign(body: str) -> bool | None:
    """Check that calc_flags_automatically appears before assign in the array.
    Returns True if correct, False if wrong, None if not applicable."""
    if 'calc_flags' not in body:
        return None
    lines = body.split('\n')
    calc_line = -1
    assign_line = -1
    for idx, line in enumerate(lines):
        if 'calc_flags_automatically' in line and calc_line == -1:
            calc_line = idx
        if '].into()' in line:
            # Check the array contents for ordering
            pass
    # Check in the final array expression
    final_match = re.search(r'\[([^\]]+)\]\.into\(\)', body)
    if final_match:
        items = final_match.group(1)
        calc_pos = items.find('calc_flags')
        assign_pos = items.find('assignment')
        if calc_pos >= 0 and assign_pos >= 0:
            return calc_pos < assign_pos
    return None


def _extract_operator(body: str) -> str | None:
    """Extract the primary IR operator from the body (b::add, b::sub, etc.)."""
    m = re.search(r'(b::\w+|u::\w+)\(', body)
    return m.group(1) if m else None


def _pseudocode_implies_op(pseudocode: str) -> str | None:
    """Infer expected IR operator from pseudocode keywords."""
    pc = pseudocode.lower()
    # Intel patterns
    if ':= dest + src' in pc or ':= dest + src' in pseudocode:
        return 'b::add'
    if ':= dest - src' in pc or ':= dest - (src' in pc:
        return 'b::sub'
    if ':= dest and src' in pc:
        return 'b::and'
    if ':= dest or src' in pc:
        return 'b::or'
    if ':= dest xor src' in pc:
        return 'b::xor'
    # ARM patterns
    if 'addwithcarry(' in pc:
        # NOT(operand2) means subtraction
        if 'not(operand2)' in pc or 'not(op2)' in pc or 'not(shifted)' in pc:
            return 'b::sub'
        return 'b::add'
    if 'operand1 eor operand2' in pc:
        return 'b::xor'
    if 'operand1 and operand2' in pc:
        return 'b::and'
    if 'operand1 or operand2' in pc:
        return 'b::or'
    return None


# ============================================================
# Per-instruction semantic validators
# ============================================================

def validate_intel_function(fn: FunctionIR) -> ValidationResult:
    """Validate a single Intel function's IR against its pseudocode."""
    name = fn.name
    body = fn.body
    pc = fn.pseudocode.lower() if fn.pseudocode else ''
    issues: list[str] = []

    # --- Structural checks ---

    # 1. calc_flags ordering
    flags_order = _check_flags_before_assign(body)
    if flags_order is False:
        issues.append("calc_flags_automatically AFTER assign (should be before)")

    # 2. Check for unknown_data() usage
    if 'unknown_data()' in body:
        issues.append("contains unknown_data() — expression could not be parsed")

    # --- Operator correctness ---

    # Check that pseudocode operation matches IR operator
    if pc:
        # ADD: DEST := DEST + SRC
        if 'dest := dest + src' in pc or 'dest ← dest + src' in pc:
            if 'b::add' not in body:
                issues.append(f"pseudocode implies ADD but body lacks b::add")

        # SUB: DEST := DEST - SRC
        if 'dest := dest - src' in pc or 'dest := dest - (src' in pc:
            if 'b::sub' not in body:
                issues.append(f"pseudocode implies SUB but body lacks b::sub")

        # AND/OR/XOR
        if 'dest := dest and src' in pc and 'b::and' not in body:
            issues.append("pseudocode implies AND but body lacks b::and")
        if 'dest := dest or src' in pc and 'b::or' not in body:
            issues.append("pseudocode implies OR but body lacks b::or")
        if 'dest := dest xor src' in pc and 'b::xor' not in body:
            issues.append("pseudocode implies XOR but body lacks b::xor")

    # --- Flag checks for known instruction categories ---

    # Arithmetic instructions should have all 6 flags
    arith_names = {'add', 'sub', 'adc', 'adcx', 'adox', 'sbb', 'inc', 'dec', 'neg', 'cmp'}
    if name in arith_names and 'calc_flags_automatically' in body:
        # Check it affects the right flags
        if name in ('inc', 'dec'):
            # INC/DEC don't affect CF
            if '&cf' in body and 'calc_flags' in body:
                # Check if cf is in the calc_flags call specifically
                calc_match = re.search(r'calc_flags_automatically\([^)]+,\s*&\[([^\]]+)\]', body)
                if calc_match and '&cf' in calc_match.group(1):
                    issues.append(f"{name} should not affect CF in calc_flags")
        else:
            if 'calc_flags_automatically' in body:
                calc_match = re.search(r'calc_flags_automatically\([^)]+,\s*&\[([^\]]+)\]', body)
                if calc_match:
                    flags_str = calc_match.group(1)
                    for flag in ['&of', '&sf', '&zf', '&cf', '&pf']:
                        if flag not in flags_str and name not in ('inc', 'dec'):
                            issues.append(f"{name}: missing {flag} in calc_flags")

    # Logical instructions should clear OF/CF
    logic_names = {'and', 'or', 'xor', 'test'}
    if name in logic_names:
        if 'calc_flags_automatically' in body:
            calc_match = re.search(r'calc_flags_automatically\([^)]+,\s*&\[([^\]]+)\]', body)
            if calc_match:
                flags_str = calc_match.group(1)
                # Should only have sf, zf, pf in calc_flags (not of, cf)
                if '&of' in flags_str:
                    issues.append(f"{name}: OF should not be in calc_flags (should be explicitly cleared)")
                if '&cf' in flags_str:
                    issues.append(f"{name}: CF should not be in calc_flags (should be explicitly cleared)")

    # --- Specific instruction checks ---

    # MOV should use zero_extend or sign_extend
    if name == 'mov' and 'zero_extend' not in body and 'sign_extend' not in body:
        if 'assign(o2()' in body:
            issues.append("MOV should use u::zero_extend(o2()) per fireman reference")

    # PUSH/POP stack pointer management
    if name == 'push':
        if 'b::sub' not in body and 'rsp' not in body and 'sp' not in body:
            issues.append("PUSH should decrement stack pointer")
    if name == 'pop':
        if 'b::add' not in body and 'rsp' not in body and 'sp' not in body:
            issues.append("POP should increment stack pointer")

    # CALL should use jump_by_call
    if name == 'call' and 'jump_by_call' not in body and 'jump(' not in body:
        issues.append("CALL should use jump_by_call or jump")

    # RET should jump to return address
    if name == 'ret' and 'jump(' not in body:
        issues.append("RET should use jump()")

    # JMP
    if name == 'jmp' and 'jump(' not in body:
        issues.append("JMP should use jump()")

    # Determine verdict
    if not issues:
        return ValidationResult(name, "CORRECT", arch='intel')
    elif all('unknown_data' in i or 'should use u::zero_extend' in i for i in issues):
        return ValidationResult(name, "ACCEPTABLE", issues, arch='intel')
    else:
        return ValidationResult(name, "WRONG", issues, arch='intel')


def validate_arm_function(fn: FunctionIR) -> ValidationResult:
    """Validate a single ARM function's IR against its pseudocode."""
    name = fn.name
    body = fn.body
    pc = fn.pseudocode.lower() if fn.pseudocode else ''
    issues: list[str] = []

    # --- Structural checks ---
    flags_order = _check_flags_before_assign(body)
    if flags_order is False:
        issues.append("calc_flags_automatically AFTER assign (should be before)")

    if 'unknown_data()' in body:
        issues.append("contains unknown_data() — expression could not be parsed")

    # --- Operator correctness from pseudocode ---
    if pc:
        # AddWithCarry pattern — use instruction name to determine add vs sub
        # Shared pseudocode has "if sub_op then operand2 = NOT(operand2)"
        # for both add and sub variants, so we can't rely on NOT substring
        if 'addwithcarry(' in pc:
            _arm_sub_instructions = {'sub', 'subs', 'sbc', 'sbcs', 'cmp', 'ccmp', 'negs', 'ngcs', 'subp', 'subps'}
            if name in _arm_sub_instructions:
                if 'b::sub' not in body:
                    issues.append("pseudocode has AddWithCarry (subtraction variant) but body lacks b::sub")
            else:
                if 'b::add' not in body:
                    issues.append("pseudocode has AddWithCarry but body lacks b::add")

        # Shared logical pseudocode: "case op of" pattern — use instruction name
        # to determine which branch applies, not substring matching
        _arm_logical_op_map = {
            'and': 'b::and', 'ands': 'b::and', 'bic': 'b::and', 'bics': 'b::and',
            'orr': 'b::or', 'orn': 'b::or',
            'eor': 'b::xor', 'eon': 'b::xor',
        }
        if name in _arm_logical_op_map:
            expected_op = _arm_logical_op_map[name]
            if expected_op not in body:
                issues.append(f"pseudocode implies {expected_op} for {name} but not found in body")
        else:
            # Non-shared pseudocode: direct matching is safe
            if 'operand1 eor operand2' in pc and 'case op' not in pc and 'b::xor' not in body:
                issues.append("pseudocode has EOR but body lacks b::xor")
            if 'operand1 and operand2' in pc and 'case op' not in pc and 'b::and' not in body:
                issues.append("pseudocode has AND but body lacks b::and")

    # --- ARM-specific instruction checks ---

    # Flag-setting instructions (name ends with 's')
    flag_setters = {'adds', 'subs', 'adcs', 'sbcs', 'ands', 'bics', 'negs', 'ngcs'}
    if name in flag_setters:
        if 'calc_flags_automatically' not in body:
            issues.append(f"{name} should set NZCV flags (missing calc_flags_automatically)")
        else:
            calc_match = re.search(r'calc_flags_automatically\([^)]+,\s*&\[([^\]]+)\]', body)
            if calc_match:
                flags_str = calc_match.group(1)
                for flag in ['&pstate_n', '&pstate_z', '&pstate_c', '&pstate_v']:
                    if flag not in flags_str:
                        issues.append(f"{name}: missing {flag} in calc_flags")

    # ADC/ADCS should include carry
    if name in ('adc', 'adcs'):
        if 'pstate_c' not in body:
            issues.append(f"{name} should include pstate_c (carry flag)")

    # SBC/SBCS should include carry
    if name in ('sbc', 'sbcs'):
        if 'pstate_c' not in body:
            issues.append(f"{name} should include pstate_c (carry flag)")

    # Branch instructions
    if name == 'b' and 'jump(' not in body:
        issues.append("B should use jump()")
    if name == 'bl':
        if 'x30' not in body:
            issues.append("BL should save return address to x30 (LR)")
        if 'jump(' not in body:
            issues.append("BL should use jump()")
    if name == 'ret' and 'jump(' not in body:
        issues.append("RET should use jump()")
    if name == 'ret' and 'x30' not in body:
        issues.append("RET should jump to x30 (LR)")

    # CBZ/CBNZ should use condition + jump with zero-compare
    if name in ('cbz', 'cbnz'):
        if 'condition(' not in body:
            issues.append(f"{name} should use condition() for conditional branch")
        if 'jump(' not in body:
            issues.append(f"{name} should use jump()")
        if 'c(0)' not in body and 'equal' not in body:
            issues.append(f"{name} should compare against zero")
        if 'fallthrough' not in body or 'instruction_byte_size()' not in body:
            issues.append(f"{name} should compute an explicit fallthrough target")
        if 'jump(o2())' not in body:
            issues.append(f"{name} should branch to o2()")

    # TBZ/TBNZ should use condition + jump with bit-test (shift+mask)
    if name in ('tbz', 'tbnz'):
        if 'condition(' not in body:
            issues.append(f"{name} should use condition() for conditional branch")
        if 'jump(' not in body:
            issues.append(f"{name} should use jump()")
        if 'b::shr' not in body and 'b::and' not in body:
            issues.append(f"{name} should use shift+mask for bit test")
        if 'fallthrough' not in body or 'instruction_byte_size()' not in body:
            issues.append(f"{name} should compute an explicit fallthrough target")
        if 'jump(o3())' not in body:
            issues.append(f"{name} should branch to o3()")

    # Mov-like load/store aliases should keep the same operand wiring as the generator templates.
    if name in {'ldr', 'ldrb', 'ldrh', 'ldar', 'ldarb', 'ldarh', 'ldur', 'ldurb', 'ldurh'}:
        if 'assign(o2(), o1(), o1_size())' not in body:
            issues.append(f"{name} should assign o2() into o1() using o1_size()")

    if name in {'str', 'strb', 'strh', 'stlr', 'stlrb', 'stlrh', 'stur'}:
        if 'assign(o1(), o2(), o2_size())' not in body:
            issues.append(f"{name} should assign o1() into o2() using o2_size()")

    if name in {'prfm', 'prfum', 'dgh', 'yield'}:
        if '[].into()' not in body:
            issues.append(f"{name} should remain a no-op template")

    arm_mov_like = {
        'ldr', 'ldrb', 'ldrh', 'ldar', 'ldarb', 'ldarh', 'ldxr', 'ldxrb', 'ldxrh',
        'ldaxr', 'ldaxrb', 'ldaxrh', 'ldxp', 'ldaxp', 'ldur', 'ldurb', 'ldurh', 'ldp', 'ldnp',
        'ldapr', 'ldaprb', 'ldaprh', 'ldapur', 'ldapurb', 'ldapurh',
        'ldlar', 'ldlarb', 'ldlarh', 'ldtr', 'ldtrb', 'ldtrh',
    }
    if name in arm_mov_like:
        if 'assign(' not in body:
            issues.append(f"{name} should use assign()")
        if 'o2()' not in body or 'o1()' not in body:
            issues.append(f"{name} should move source operand into destination operand")

    arm_signext_loads = {
        'ldrsb', 'ldrsh', 'ldrsw', 'ldursb', 'ldursh', 'ldursw',
        'ldapursb', 'ldapursh', 'ldapursw', 'ldtrsb', 'ldtrsh', 'ldtrsw',
    }
    if name in arm_signext_loads and 'sign_extend(' not in body:
        issues.append(f"{name} should use sign_extend()")

    arm_store_like = {
        'str', 'strb', 'strh', 'stlr', 'stlrb', 'stlrh', 'stxr', 'stxrb', 'stxrh',
        'stlxr', 'stlxrb', 'stlxrh', 'stxp', 'stlxp', 'stur', 'sturb', 'sturh',
        'stllr', 'stllrb', 'stllrh', 'stlur', 'stlurb', 'stlurh',
        'stp', 'stnp', 'sttr', 'sttrb', 'sttrh',
    }
    if name in arm_store_like:
        if 'assign(' not in body:
            issues.append(f"{name} should use assign()")
        if 'o1(), o2(), o2_size()' not in body:
            issues.append(f"{name} should store operand1 into operand2")

    if name == 'rev':
        if 'tmp32' not in body and 'tmp64' not in body:
            issues.append("rev should use byte-swap temporaries")
        if 'b::shl' not in body or 'b::shr' not in body:
            issues.append("rev should use shift-based byte reversal")
        if 'condition(' not in body or 'bit_size_of_o1()' not in body:
            issues.append("rev should branch on operand width")

    if name == 'rmif':
        if 'tmp64' not in body:
            issues.append("rmif should materialize the rotated source into tmp64")
        if 'condition(' not in body:
            issues.append("rmif should gate flag updates with condition()")
        if 'o2()' not in body or 'o3()' not in body:
            issues.append("rmif should use the shift and mask operands")
        if 'b::shr' not in body or 'b::and' not in body:
            issues.append("rmif should extract rotated low bits with shift-and-mask operations")
        for flag in ['pstate_n', 'pstate_z', 'pstate_c', 'pstate_v']:
            if flag not in body:
                issues.append(f"rmif should conditionally update {flag}")
        if re.search(r'assign\([^,]+,\s*o1\(\)', body):
            issues.append("rmif should not write back into the source register")

    # AXFLAG/XAFLAG should assign all 4 PSTATE flags
    if name in ('axflag', 'xaflag'):
        for flag in ['pstate_n', 'pstate_z', 'pstate_c', 'pstate_v']:
            if flag not in body:
                issues.append(f"{name} should assign {flag}")

    # ADRP should page-align PC
    if name == 'adrp':
        if '0xFFF' not in body and '0xfff' not in body.lower():
            issues.append("ADRP should mask low 12 bits of PC (page alignment)")

    # Conditional select should use condition()
    csel_family = {'csel', 'csinc', 'csinv', 'csneg'}
    if name in csel_family:
        if 'condition(' not in body:
            issues.append(f"{name} should use condition() for conditional select")

    # Non-flag-setting arithmetic should NOT have calc_flags
    no_flag_arith = {'add', 'sub', 'and', 'orr', 'eor', 'mul'}
    if name in no_flag_arith and 'calc_flags_automatically' in body:
        issues.append(f"{name} (no S suffix) should NOT set flags")

    # CMP/CMN should only set flags (no assignment to destination)
    if name in ('cmp', 'cmn', 'tst', 'ccmp', 'ccmn'):
        if 'calc_flags_automatically' not in body:
            issues.append(f"{name} should set flags")
        # Should not assign to o1() as destination
        assign_match = re.search(r'assign\([^,]+,\s*o1\(\)', body)
        if assign_match:
            issues.append(f"{name} should not write to destination register")

    # Determine verdict
    if not issues:
        return ValidationResult(name, "CORRECT", arch='arm')
    elif all('unknown_data' in i for i in issues):
        return ValidationResult(name, "ACCEPTABLE", issues, arch='arm')
    else:
        # Distinguish ACCEPTABLE (minor) from WRONG (semantic)
        serious = [i for i in issues if 'unknown_data' not in i and 'should use u::zero_extend' not in i]
        if not serious:
            return ValidationResult(name, "ACCEPTABLE", issues, arch='arm')
        return ValidationResult(name, "WRONG", issues, arch='arm')


# ============================================================
# File-level validation
# ============================================================

def validate_file(filepath: str, arch: str) -> tuple[str, list[ValidationResult]]:
    """Validate all functions in a single generated .rs file."""
    functions = parse_generated_rs(filepath, arch)
    results: list[ValidationResult] = []

    for fn in functions:
        if fn.is_exception:
            results.append(ValidationResult(fn.name, "EXCEPTION_STUB", arch=arch))
            continue

        if arch == 'intel':
            result = validate_intel_function(fn)
        else:
            result = validate_arm_function(fn)
        results.append(result)

    return filepath, results


# ============================================================
# Main
# ============================================================

def main():
    base_dir = os.path.dirname(os.path.abspath(__file__))
    output_dir = os.path.join(base_dir, "output")

    intel_files = sorted(glob(os.path.join(output_dir, "intel", "*_generated.rs")))
    arm_files = sorted(glob(os.path.join(output_dir, "arm", "*_generated.rs")))

    all_tasks = [(f, 'intel') for f in intel_files] + [(f, 'arm') for f in arm_files]

    all_results: dict[str, list[ValidationResult]] = {}

    # Parallel execution
    with ThreadPoolExecutor(max_workers=min(8, len(all_tasks))) as executor:
        futures = {executor.submit(validate_file, f, a): (f, a) for f, a in all_tasks}
        for future in as_completed(futures):
            filepath, results = future.result()
            all_results[filepath] = results

    # Report
    total_by_arch: dict[str, dict[str, int]] = {
        'intel': {'CORRECT': 0, 'ACCEPTABLE': 0, 'WRONG': 0, 'EXCEPTION_STUB': 0},
        'arm': {'CORRECT': 0, 'ACCEPTABLE': 0, 'WRONG': 0, 'EXCEPTION_STUB': 0},
    }

    wrong_details: list[tuple[str, ValidationResult]] = []

    for filepath in sorted(all_results.keys()):
        results = all_results[filepath]
        rel_path = os.path.relpath(filepath, base_dir)
        non_exception = [r for r in results if r.verdict != 'EXCEPTION_STUB']
        exception_count = len(results) - len(non_exception)

        counts = {}
        for r in results:
            counts[r.verdict] = counts.get(r.verdict, 0) + 1
            total_by_arch[r.arch][r.verdict] += 1

        # Print file header
        parts = []
        for v in ['CORRECT', 'ACCEPTABLE', 'WRONG', 'EXCEPTION_STUB']:
            if counts.get(v, 0) > 0:
                parts.append(f"{v}: {counts[v]}")
        print(f"=== {rel_path} ({len(results)} funcs) === {', '.join(parts)}")

        # Print WRONG details
        for r in results:
            if r.verdict == 'WRONG':
                print(f"  WRONG: {r.name}")
                for issue in r.issues:
                    print(f"    - {issue}")
                wrong_details.append((rel_path, r))

    # Summary
    print("\n" + "=" * 60)
    print("VALIDATION SUMMARY")
    print("=" * 60)

    for arch in ['intel', 'arm']:
        totals = total_by_arch[arch]
        real_ir = totals['CORRECT'] + totals['ACCEPTABLE'] + totals['WRONG']
        total = real_ir + totals['EXCEPTION_STUB']
        print(f"\n{arch.upper()} ({total} total functions):")
        print(f"  Real IR:        {real_ir}")
        print(f"    CORRECT:      {totals['CORRECT']}")
        print(f"    ACCEPTABLE:   {totals['ACCEPTABLE']}")
        print(f"    WRONG:        {totals['WRONG']}")
        print(f"  Exception stubs: {totals['EXCEPTION_STUB']}")

    total_wrong = total_by_arch['intel']['WRONG'] + total_by_arch['arm']['WRONG']
    if total_wrong == 0:
        print("\nAll real IR functions passed validation!")
    else:
        print(f"\n{total_wrong} functions have semantic issues (see details above)")

    return 0 if total_wrong == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
