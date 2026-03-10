#!/usr/bin/env python3
"""
Validate generated IR stubs against fireman's manually-implemented versions.

Reads the generated output/intel/*.rs files and compares the IR structure
against the known-correct implementations in fireman's instruction_analyze/*.rs.

Checks:
1. Semantic structure match (same IR operators and operands used)
2. Flag handling correctness (same flags affected)
3. No todo!() remaining
"""

import re
import os
import sys
from dataclasses import dataclass


FIREMAN_DIR = ""
GENERATED_DIR = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "output", "intel"
)
EXPECTED_JCC_FUNCTIONS = (
    "ja",
    "jae",
    "jb",
    "jbe",
    "jc",
    "je",
    "jcxz",
    "jecxz",
    "jrcxz",
    "jg",
    "jge",
    "jl",
    "jle",
    "jna",
    "jnae",
    "jnb",
    "jnbe",
    "jnc",
    "jne",
    "jng",
    "jnge",
    "jnl",
    "jnle",
    "jno",
    "jnp",
    "jns",
    "jnz",
    "jo",
    "jp",
    "jpe",
    "jpo",
    "js",
    "jz",
)


@dataclass
class IrSignature:
    """Normalized semantic signature of an IR function body."""

    operators: list[str]  # IR operators used: b::add, b::sub, u::not, etc.
    statements: list[str]  # Statement types: assign, condition, jump, calc_flags, etc.
    flags: list[str]  # Flags referenced: cf, zf, sf, of, af, pf
    operands: list[str]  # Operands used: o1(), o2(), o3(), etc.
    registers: list[str]  # Explicit registers: rax, rsp, etc.


def extract_functions(filepath: str) -> dict[str, str]:
    """Extract all pub(super) fn bodies from a Rust file."""
    with open(filepath, "r") as f:
        content = f.read()

    functions: dict[str, str] = {}
    # Match pub(super) fn name() -> ... { ... }
    # Also match #[box_to_static_reference] decorated functions
    pattern = re.compile(
        r"(?:#\[box_to_static_reference\]\s*)?"
        r"pub\(super\)\s+fn\s+(\w+)\(\)\s*->\s*[^{]+\{([^}]*(?:\{[^}]*\}[^}]*)*)\}",
        re.DOTALL,
    )
    for m in pattern.finditer(content):
        name = m.group(1)
        body = m.group(2).strip()
        functions[name] = body

    return functions


def extract_signature(body: str) -> IrSignature:
    """Extract a normalized semantic signature from a function body."""
    operators = sorted(set(re.findall(r"[bu]::\w+", body)))
    statements = sorted(
        set(
            re.findall(
                r"\b(assign|condition|jump|jump_by_call|halt|exception|calc_flags_automatically|extend_undefined_flags|type_specified|assertion)\b",
                body,
            )
        )
    )
    flags = sorted(set(re.findall(r"\b(cf|zf|sf|of|af|pf|df|tf)\b", body)))
    operands = sorted(set(re.findall(r"\bo[1-4]\(\)", body)))
    registers = sorted(
        set(
            re.findall(
                r"\b(rax|rbx|rcx|rdx|rsp|rbp|rsi|rdi|rip|eax|ebx|ecx|edx|esp|ebp|esi|edi|ax|bx|cx|dx|al|ah|bl|bh|cl|ch|dl|dh|r8|r9|r10|r11|r12|r13|r14|r15|tmp\d+|xmm\d+|ymm\d+|zmm\d+)\b",
                body,
            )
        )
    )

    return IrSignature(
        operators=operators,
        statements=statements,
        flags=flags,
        operands=operands,
        registers=registers,
    )


def compare_signatures(name: str, expected: IrSignature, got: IrSignature) -> list[str]:
    """Compare two IR signatures and return list of differences."""
    diffs: list[str] = []

    # Check operators
    expected_ops = set(expected.operators)
    got_ops = set(got.operators)
    missing_ops = expected_ops - got_ops
    extra_ops = got_ops - expected_ops
    if missing_ops:
        diffs.append(f"  Missing operators: {', '.join(sorted(missing_ops))}")
    if extra_ops:
        diffs.append(f"  Extra operators: {', '.join(sorted(extra_ops))}")

    # Check statement types
    expected_stmts = set(expected.statements)
    got_stmts = set(got.statements)
    missing_stmts = expected_stmts - got_stmts
    if missing_stmts:
        diffs.append(f"  Missing statements: {', '.join(sorted(missing_stmts))}")

    # Check flags
    expected_flags = set(expected.flags)
    got_flags = set(got.flags)
    missing_flags = expected_flags - got_flags
    if missing_flags:
        diffs.append(f"  Missing flags: {', '.join(sorted(missing_flags))}")

    # Check operands
    expected_operands = set(expected.operands)
    got_operands = set(got.operands)
    missing_operands = expected_operands - got_operands
    if missing_operands:
        diffs.append(f"  Missing operands: {', '.join(sorted(missing_operands))}")

    return diffs


def main():
    if not os.path.isdir(FIREMAN_DIR):
        print(f"ERROR: Fireman directory not found: {FIREMAN_DIR}")
        sys.exit(1)
    if not os.path.isdir(GENERATED_DIR):
        print(f"ERROR: Generated directory not found: {GENERATED_DIR}")
        sys.exit(1)

    # Extract all fireman reference implementations
    reference: dict[str, str] = {}
    for fname in os.listdir(FIREMAN_DIR):
        if fname.endswith(".rs"):
            fpath = os.path.join(FIREMAN_DIR, fname)
            funcs = extract_functions(fpath)
            reference.update(funcs)

    # Extract all generated implementations
    generated: dict[str, str] = {}
    for fname in os.listdir(GENERATED_DIR):
        if fname.endswith("_generated.rs"):
            fpath = os.path.join(GENERATED_DIR, fname)
            funcs = extract_functions(fpath)
            generated.update(funcs)

    missing_jcc = [name for name in EXPECTED_JCC_FUNCTIONS if name not in generated]
    if missing_jcc:
        print("\nERROR: Missing expected Jcc generated functions:")
        for name in missing_jcc:
            print(f"  - {name}")
        sys.exit(1)

    print(f"Reference implementations: {len(reference)}")
    print(f"Generated implementations: {len(generated)}")

    # Compare overlapping functions
    common = set(reference.keys()) & set(generated.keys())
    # Handle std_ vs std naming (fireman uses std_ to avoid Rust keyword)
    if "std_" in reference and "std" in generated:
        common.add("std")

    print(f"Common functions to validate: {len(common)}")
    print()

    passed = 0
    failed = 0
    warnings = 0
    failures: list[tuple[str, list[str]]] = []

    for name in sorted(common):
        ref_name = name if name in reference else f"{name}_"
        gen_name = name

        if ref_name not in reference or gen_name not in generated:
            continue

        ref_sig = extract_signature(reference[ref_name])
        gen_sig = extract_signature(generated[gen_name])

        diffs = compare_signatures(name, ref_sig, gen_sig)

        if not diffs:
            passed += 1
        else:
            # Check severity: missing core operators is a failure,
            # extra operators or missing type_specified is a warning
            is_failure = any(
                "Missing operators" in d
                or "Missing statements" in d
                or "Missing flags" in d
                for d in diffs
            )
            if is_failure:
                failed += 1
                failures.append((name, diffs))
            else:
                warnings += 1

    print("=" * 60)
    print(f"RESULTS: {passed} passed, {failed} failed, {warnings} warnings")
    print(f"  out of {len(common)} validated functions")
    print("=" * 60)

    if failures:
        print("\nFAILURES:")
        for name, diffs in failures:
            print(f"\n  {name}:")
            for d in diffs:
                print(f"    {d}")

    # Also check for todo!() in generated files
    print("\n--- todo!() check ---")
    todo_count = 0
    for fname in os.listdir(GENERATED_DIR):
        if fname.endswith("_generated.rs"):
            fpath = os.path.join(GENERATED_DIR, fname)
            with open(fpath) as f:
                content = f.read()
            count = content.count("todo!")
            if count > 0:
                print(f"  {fname}: {count} todo!() found")
                todo_count += count

    if todo_count == 0:
        print("  No todo!() found in any generated file.")
    else:
        print(f"  TOTAL: {todo_count} todo!() remaining")

    # Return exit code
    sys.exit(1 if failed > 0 or todo_count > 0 else 0)


if __name__ == "__main__":
    main()
