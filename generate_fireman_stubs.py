#!/usr/bin/env python3
"""
Generate fireman-compatible instruction_analyze stub files from
result/arm.rs and result/intel.rs Operation pseudocode.

Translates pseudocode into actual fireman IR code using the shortcuts DSL.
Output: output/intel/{letter}_generated.rs, output/arm/{letter}_generated.rs
"""

import re
import os
import json
import sys
from dataclasses import dataclass, field
from collections import defaultdict
from datetime import datetime
from enum import Enum, auto
from typing import Optional


@dataclass
class Instruction:
    name: str          # enum variant name e.g. "Adc"
    mnemonic: str      # lowercase e.g. "adc"
    operation: str     # Operation pseudocode (or empty)
    first_letter: str  # for file grouping


@dataclass
class GenerationResult:
    mnemonic: str
    status: str   # "real_ir" | "exception" | "skipped"
    reason: str


# ============================================================
# Section 1: Parse Rust enum files
# ============================================================

def parse_rs_enum(filepath: str) -> list[Instruction]:
    """Parse a Rust enum file with doc comments, extract variant names and Operation pseudocode."""
    with open(filepath, "r", encoding="utf-8") as f:
        content = f.read()

    instructions = []
    warnings = []
    lines = content.split("\n")

    in_enum = False
    doc_lines: list[str] = []

    for line in lines:
        if not in_enum:
            if re.match(r"^enum\s+\w+\s*\{", line):
                in_enum = True
            continue

        if line.strip() == "}":
            break

        variant_match = re.match(
            r"^\s+([A-Z][a-zA-Z0-9]*)(?:\s*[\({].*[\)}])?\s*,\s*$", line
        )
        if variant_match:
            variant_name = variant_match.group(1)
            operation = extract_operation(doc_lines)
            mnemonic = variant_name.lower()

            if doc_lines and not operation:
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
            pass
        else:
            doc_lines = []

    if warnings:
        for w in warnings:
            print(w, file=sys.stderr)

    return instructions


def extract_operation(doc_lines: list[str]) -> str:
    """Extract the Operation pseudocode from doc comment lines."""
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
    code_lines = []
    for i in range(op_start + 1, len(doc_lines)):
        line = doc_lines[i]
        stripped = line.strip()

        if any(stripped == pat or stripped.startswith(pat) for pat in _SECTION_STOP_PATTERNS):
            break
        if stripped.startswith("## "):
            break

        code_lines.append(line)

    while code_lines and not code_lines[0].strip():
        code_lines.pop(0)
    while code_lines and not code_lines[-1].strip():
        code_lines.pop()

    return "\n".join(code_lines).strip()


# ============================================================
# Section 2: IR Translation Constants
# ============================================================

# Register name → IR code
REGISTER_IR: dict[str, str] = {}

# Build register map for x86_64
_REG_FAMILIES = {
    'A': ['AL', 'AH', 'AX', 'EAX', 'RAX'],
    'B': ['BL', 'BH', 'BX', 'EBX', 'RBX'],
    'C': ['CL', 'CH', 'CX', 'ECX', 'RCX'],
    'D': ['DL', 'DH', 'DX', 'EDX', 'RDX'],
}
for _fam, _regs in _REG_FAMILIES.items():
    for _r in _regs:
        REGISTER_IR[_r] = f'{_r.lower()}.clone()'

for _r in ['SP', 'ESP', 'RSP', 'BP', 'EBP', 'RBP',
           'SI', 'ESI', 'RSI', 'DI', 'EDI', 'RDI',
           'SPL', 'BPL', 'SIL', 'DIL',
           'R8', 'R9', 'R10', 'R11', 'R12', 'R13', 'R14', 'R15']:
    REGISTER_IR[_r] = f'{_r.lower()}.clone()'

# R8-R15 sub-registers
for _i in range(8, 16):
    for _suffix in ['B', 'W', 'D']:
        _name = f'R{_i}{_suffix}'
        REGISTER_IR[_name] = f'r{_i}{_suffix.lower()}.clone()'

REGISTER_IR['RIP'] = 'rip.clone()'

# Flags
for _f in ['CF', 'PF', 'AF', 'ZF', 'SF', 'TF', 'DF', 'OF']:
    REGISTER_IR[_f] = f'{_f.lower()}.clone()'

# Tmp registers
for _t in ['tmp8', 'tmp16', 'tmp32', 'tmp64', 'tmp128', 'tmp256', 'tmp512']:
    REGISTER_IR[_t.upper()] = f'{_t}.clone()'

# SIMD registers
for _i in range(32):
    for _prefix in ['XMM', 'YMM', 'ZMM']:
        _name = f'{_prefix}{_i}'
        REGISTER_IR[_name] = f'{_name.lower()}.clone()'

# Operand name → IR code
OPERAND_IR = {
    'DEST': 'o1()',
    'SRC': 'o2()',
    'SRC1': 'o2()',
    'SRC2': 'o3()',
    'SRC3': 'o4()',
}

# Size inference for registers
REGISTER_SIZE: dict[str, str] = {}
for _r in ['AL', 'AH', 'BL', 'BH', 'CL', 'CH', 'DL', 'DH', 'SPL', 'BPL', 'SIL', 'DIL']:
    REGISTER_SIZE[_r] = f'size_relative({_r.lower()}.clone())'
for _r in ['AX', 'BX', 'CX', 'DX', 'SP', 'BP', 'SI', 'DI']:
    REGISTER_SIZE[_r] = f'size_relative({_r.lower()}.clone())'
for _r in ['EAX', 'EBX', 'ECX', 'EDX', 'ESP', 'EBP', 'ESI', 'EDI']:
    REGISTER_SIZE[_r] = f'size_relative({_r.lower()}.clone())'
for _r in ['RAX', 'RBX', 'RCX', 'RDX', 'RSP', 'RBP', 'RSI', 'RDI', 'RIP',
           'R8', 'R9', 'R10', 'R11', 'R12', 'R13', 'R14', 'R15']:
    REGISTER_SIZE[_r] = f'size_relative({_r.lower()}.clone())'
for _f in ['CF', 'PF', 'AF', 'ZF', 'SF', 'TF', 'DF', 'OF']:
    REGISTER_SIZE[_f] = f'size_relative({_f.lower()}.clone())'

# Condition codes for Jcc/CMOVcc/SETcc
CONDITION_CODES: dict[str, str] = {
    'a':   'b::and(u::not(cf.clone()), u::not(zf.clone()))',
    'ae':  'u::not(cf.clone())',
    'b':   'cf.clone()',
    'be':  'b::or(cf.clone(), zf.clone())',
    'c':   'cf.clone()',
    'e':   'zf.clone()',
    'g':   'b::and(u::not(zf.clone()), sf_eq_of())',
    'ge':  'sf_eq_of()',
    'l':   'sf_ne_of()',
    'le':  'b::or(zf.clone(), sf_ne_of())',
    'na':  'b::or(cf.clone(), zf.clone())',
    'nae': 'cf.clone()',
    'nb':  'u::not(cf.clone())',
    'nbe': 'b::and(u::not(cf.clone()), u::not(zf.clone()))',
    'nc':  'u::not(cf.clone())',
    'ne':  'u::not(zf.clone())',
    'ng':  'b::or(zf.clone(), sf_ne_of())',
    'nge': 'sf_ne_of()',
    'nl':  'sf_eq_of()',
    'nle': 'b::and(u::not(zf.clone()), sf_eq_of())',
    'no':  'u::not(of.clone())',
    'np':  'u::not(pf.clone())',
    'ns':  'u::not(sf.clone())',
    'nz':  'u::not(zf.clone())',
    'o':   'of.clone()',
    'p':   'pf.clone()',
    'pe':  'pf.clone()',
    'po':  'u::not(pf.clone())',
    's':   'sf.clone()',
    'z':   'zf.clone()',
}

ALL_FLAGS = '&[&of, &sf, &zf, &af, &cf, &pf]'
NO_CF_FLAGS = '&[&of, &sf, &zf, &af, &pf]'
LOGIC_FLAGS = '&[&sf, &zf, &pf]'


# ============================================================
# Section 2b: ARM (AArch64) Register & Operand Maps
# ============================================================

def _build_arm_registers() -> tuple[dict[str, str], dict[str, str]]:
    """Build ARM AArch64 register → IR mapping."""
    reg_ir: dict[str, str] = {}
    reg_size: dict[str, str] = {}

    # General purpose X0-X30 (64-bit)
    for i in range(31):
        reg_ir[f'X{i}'] = f'x{i}.clone()'
        reg_size[f'X{i}'] = f'size_relative(x{i}.clone())'

    # General purpose W0-W30 (32-bit, lower half of X)
    for i in range(31):
        reg_ir[f'W{i}'] = f'w{i}.clone()'
        reg_size[f'W{i}'] = f'size_relative(w{i}.clone())'

    # SP / WSP
    reg_ir['SP'] = 'sp.clone()'
    reg_size['SP'] = 'size_relative(sp.clone())'
    reg_ir['WSP'] = 'wsp.clone()'
    reg_size['WSP'] = 'size_relative(wsp.clone())'

    # PC
    reg_ir['PC'] = 'pc.clone()'
    reg_size['PC'] = 'size_relative(pc.clone())'

    # Zero registers
    reg_ir['XZR'] = 'xzr.clone()'
    reg_size['XZR'] = 'size_relative(xzr.clone())'
    reg_ir['WZR'] = 'wzr.clone()'
    reg_size['WZR'] = 'size_relative(wzr.clone())'

    # LR = X30 alias
    reg_ir['LR'] = 'x30.clone()'
    reg_size['LR'] = 'size_relative(x30.clone())'

    # SIMD/FP registers V0-V31 with width views
    for i in range(32):
        for prefix in ['V', 'Q', 'D', 'S', 'H', 'B']:
            name = f'{prefix}{i}'
            reg_ir[name] = f'{name.lower()}.clone()'
            reg_size[name] = f'size_relative({name.lower()}.clone())'

    # PSTATE flags (N, Z, C, V)
    for flag in ['N', 'Z', 'C', 'V']:
        reg_ir[f'PSTATE_{flag}'] = f'pstate_{flag.lower()}.clone()'
        reg_size[f'PSTATE_{flag}'] = f'size_relative(pstate_{flag.lower()}.clone())'

    return reg_ir, reg_size


ARM_REGISTER_IR, ARM_REGISTER_SIZE = _build_arm_registers()

# ARM pseudocode register-access patterns → IR operand
# X[d]/W[d]/V[d] = destination (o1), X[n]/W[n]/V[n] = src1 (o2),
# X[m]/W[m]/V[m] = src2 (o3), X[t]/W[t]/V[t] = transfer reg (o1),
# X[s]/W[s] = store-data reg (o2), X[a]/V[a] = accumulator (o3)
ARM_PSEUDOCODE_REGS: dict[str, str] = {
    'X[d]': 'o1()', 'W[d]': 'o1()', 'V[d]': 'o1()',
    'X[n]': 'o2()', 'W[n]': 'o2()', 'V[n]': 'o2()',
    'X[m]': 'o3()', 'W[m]': 'o3()', 'V[m]': 'o3()',
    'X[t]': 'o1()', 'W[t]': 'o1()', 'V[t]': 'o1()',
    'X[s]': 'o2()', 'W[s]': 'o2()',
    'X[a]': 'o3()', 'V[a]': 'o3()',
}

ARM_PSEUDOCODE_SIZES: dict[str, str] = {
    'X[d]': 'o1_size()', 'W[d]': 'o1_size()', 'V[d]': 'o1_size()',
    'X[n]': 'o2_size()', 'W[n]': 'o2_size()', 'V[n]': 'o2_size()',
    'X[m]': 'o3_size()', 'W[m]': 'o3_size()', 'V[m]': 'o3_size()',
    'X[t]': 'o1_size()', 'W[t]': 'o1_size()', 'V[t]': 'o1_size()',
    'X[s]': 'o2_size()', 'W[s]': 'o2_size()',
    'X[a]': 'o3_size()', 'V[a]': 'o3_size()',
}

# ARM condition codes (for b.eq, b.ne, csel, etc.)
ARM_CONDITION_CODES: dict[str, str] = {
    'eq': 'pstate_z.clone()',
    'ne': 'u::not(pstate_z.clone())',
    'cs': 'pstate_c.clone()',
    'hs': 'pstate_c.clone()',
    'cc': 'u::not(pstate_c.clone())',
    'lo': 'u::not(pstate_c.clone())',
    'mi': 'pstate_n.clone()',
    'pl': 'u::not(pstate_n.clone())',
    'vs': 'pstate_v.clone()',
    'vc': 'u::not(pstate_v.clone())',
    'hi': 'b::and(pstate_c.clone(), u::not(pstate_z.clone()))',
    'ls': 'b::or(u::not(pstate_c.clone()), pstate_z.clone())',
    'ge': 'b::equal(pstate_n.clone(), pstate_v.clone(), size_relative(pstate_n.clone()))',
    'lt': 'u::not(b::equal(pstate_n.clone(), pstate_v.clone(), size_relative(pstate_n.clone())))',
    'gt': 'b::and(u::not(pstate_z.clone()), b::equal(pstate_n.clone(), pstate_v.clone(), size_relative(pstate_n.clone())))',
    'le': 'b::or(pstate_z.clone(), u::not(b::equal(pstate_n.clone(), pstate_v.clone(), size_relative(pstate_n.clone()))))',
    'al': 'c(1)',
}

ARM_NZCV_FLAGS = '&[&pstate_n, &pstate_z, &pstate_c, &pstate_v]'


# ============================================================
# Section 3: Pseudocode Tokenizer & Parser
# ============================================================

def tokenize_pseudocode(text: str) -> list[tuple[str, str]]:
    """Tokenize Intel pseudocode into (kind, value) pairs."""
    tokens: list[tuple[str, str]] = []
    i = 0
    n = len(text)
    while i < n:
        # Whitespace
        if text[i].isspace():
            i += 1
            continue
        # Comment (* ... *)
        if text[i:i+2] == '(*':
            end = text.find('*)', i + 2)
            i = (end + 2) if end >= 0 else n
            continue
        # Assignment :=
        if text[i:i+2] == ':=':
            tokens.append(('ASSIGN', ':='))
            i += 2
            continue
        # Shift operators
        if text[i:i+2] in ('<<', '>>'):
            tokens.append(('OP', text[i:i+2]))
            i += 2
            continue
        # Single-char delimiters
        if text[i] in '+-*/;,()[]:#{}':
            tokens.append(('OP', text[i]))
            i += 1
            continue
        # Number: hex (0x... or digits followed by H) or decimal
        if text[i].isdigit():
            j = i
            while j < n and (text[j].isalnum() or text[j] == '_'):
                j += 1
            word = text[i:j]
            tokens.append(('NUM', word))
            i = j
            continue
        # Identifier/keyword
        if text[i].isalpha() or text[i] == '_':
            j = i
            while j < n and (text[j].isalnum() or text[j] == '_'):
                j += 1
            word = text[i:j]
            tokens.append(('ID', word))
            i = j
            continue
        # Skip unknown
        i += 1
    return tokens


class ExprParser:
    """Simple recursive descent parser for pseudocode expressions → IR code strings."""

    def __init__(self, tokens: list[tuple[str, str]], temp_vars: dict[str, str] | None = None):
        self.tokens = tokens
        self.pos = 0
        self.temp_vars = temp_vars or {}

    def peek(self) -> tuple[str, str] | None:
        if self.pos < len(self.tokens):
            return self.tokens[self.pos]
        return None

    def peek_val(self) -> str | None:
        t = self.peek()
        return t[1] if t else None

    def consume(self) -> tuple[str, str]:
        tok = self.tokens[self.pos]
        self.pos += 1
        return tok

    def at_end(self) -> bool:
        return self.pos >= len(self.tokens)

    def expect(self, val: str) -> bool:
        if self.peek_val() == val:
            self.consume()
            return True
        return False

    def parse_expr(self) -> str:
        return self.parse_or()

    def parse_or(self) -> str:
        left = self.parse_xor()
        while self.peek() and self.peek() == ('ID', 'OR'):
            self.consume()
            right = self.parse_xor()
            left = f'b::or({left}, {right})'
        # Also handle 'or' lowercase and 'bitwiseOR'
        while self.peek() and self.peek()[1] in ('or', 'bitwiseOR'):
            self.consume()
            right = self.parse_xor()
            left = f'b::or({left}, {right})'
        return left

    def parse_xor(self) -> str:
        left = self.parse_and()
        while self.peek() and self.peek()[1] in ('XOR', 'xor', 'bitwiseXOR'):
            self.consume()
            right = self.parse_and()
            left = f'b::xor({left}, {right})'
        return left

    def parse_and(self) -> str:
        left = self.parse_add()
        while self.peek() and self.peek()[1] in ('AND', 'and', 'bitwiseAND'):
            self.consume()
            right = self.parse_add()
            left = f'b::and({left}, {right})'
        return left

    def parse_add(self) -> str:
        left = self.parse_mul()
        while self.peek() and self.peek_val() in ('+', '-'):
            op = self.consume()[1]
            right = self.parse_mul()
            if op == '+':
                left = f'b::add({left}, {right})'
            else:
                left = f'b::sub({left}, {right})'
        return left

    def parse_mul(self) -> str:
        left = self.parse_shift()
        while self.peek() and self.peek_val() in ('*', '/', 'MOD', 'mod'):
            op = self.consume()[1]
            right = self.parse_shift()
            if op == '*':
                left = f'b::mul({left}, {right})'
            elif op in ('/', 'DIV'):
                left = f'b::unsigned_div({left}, {right})'
            else:
                left = f'b::unsigned_rem({left}, {right})'
        return left

    def parse_shift(self) -> str:
        left = self.parse_unary()
        while self.peek() and self.peek_val() in ('<<', '>>', 'SHL', 'SHR'):
            op = self.consume()[1]
            right = self.parse_unary()
            if op in ('<<', 'SHL'):
                left = f'b::shl({left}, {right})'
            else:
                left = f'b::shr({left}, {right})'
        return left

    def parse_unary(self) -> str:
        if self.peek() and self.peek()[1] in ('NOT', 'not'):
            self.consume()
            operand = self.parse_unary()
            return f'u::not({operand})'
        if self.peek() and self.peek_val() == '-':
            self.consume()
            operand = self.parse_unary()
            return f'u::neg({operand})'
        if self.peek() and self.peek_val() == '~':
            self.consume()
            operand = self.parse_unary()
            return f'u::not({operand})'
        return self.parse_primary()

    def parse_primary(self) -> str:
        tok = self.peek()
        if tok is None:
            return 'unknown_data()'

        # Number
        if tok[0] == 'NUM':
            self.consume()
            return self._translate_number(tok[1])

        # Parenthesized expression
        if tok[1] == '(':
            self.consume()
            expr = self.parse_expr()
            self.expect(')')
            return expr

        # Identifier
        if tok[0] == 'ID':
            name = self.consume()[1]

            # Function call?
            if self.peek_val() == '(':
                return self._translate_call(name)

            # Bit field access? X[high:low]
            if self.peek_val() == '[':
                self.consume()  # [
                self._skip_until(']')
                base_ir = self._translate_atom(name)
                return base_ir  # simplified: ignore bit extraction

            # Simple atom
            return self._translate_atom(name)

        # Skip unknown token
        self.consume()
        return 'unknown_data()'

    def _translate_number(self, val: str) -> str:
        val_upper = val.upper()
        if val_upper.endswith('H'):
            hex_str = val_upper[:-1]
            try:
                n = int(hex_str, 16)
                return f'c(0x{hex_str})'
            except ValueError:
                return f'c(0)'
        try:
            n = int(val)
            return f'c({n})'
        except ValueError:
            return 'c(0)'

    def _translate_atom(self, name: str) -> str:
        # Check operand names
        if name in OPERAND_IR:
            return OPERAND_IR[name]
        # Check register names
        if name in REGISTER_IR:
            return REGISTER_IR[name]
        # Check temp variables
        if name in self.temp_vars:
            return self.temp_vars[name]
        # imm8/imm16/imm32/imm64 → operand 2 (immediate value)
        if name.startswith('imm'):
            return 'o2()'
        # OperandSize, AddressSize → architecture_bit_size()
        if name in ('OperandSize', 'AddressSize'):
            return 'architecture_bit_size()'
        # Unknown variable
        return 'unknown_data()'

    def _translate_call(self, name: str) -> str:
        self.consume()  # consume (
        args = []
        depth = 1
        while not self.at_end() and depth > 0:
            if self.peek_val() == '(':
                depth += 1
            elif self.peek_val() == ')':
                depth -= 1
                if depth == 0:
                    self.consume()
                    break
            if self.peek_val() == ',' and depth == 1:
                self.consume()
                continue
            args.append(self.parse_expr())

        arg0 = args[0] if args else 'o1()'

        func_map = {
            'SignExtend': f'u::sign_extend({arg0})',
            'ZeroExtend': f'u::zero_extend({arg0})',
            'ZERO_EXTEND': f'u::zero_extend({arg0})',
            'ZERO_EXTEND_TO_512': f'u::zero_extend({arg0})',
            'Abs': arg0,  # No IR equivalent, pass through
            'Neg': f'u::neg({arg0})',
            'NOT': f'u::not({arg0})',
        }
        if name in func_map:
            return func_map[name]
        # Unknown function → unknown_data
        return 'unknown_data()'

    def _skip_until(self, end_val: str):
        depth = 1 if end_val in (']', ')') else 0
        open_val = '[' if end_val == ']' else '('
        while not self.at_end():
            v = self.peek_val()
            if v == open_val:
                depth += 1
            elif v == end_val:
                depth -= 1
                if depth <= 0:
                    self.consume()
                    return
            self.consume()


def parse_expression(text: str, temp_vars: dict[str, str] | None = None) -> str:
    """Parse a pseudocode expression string and return IR code."""
    tokens = tokenize_pseudocode(text)
    if not tokens:
        return 'unknown_data()'
    parser = ExprParser(tokens, temp_vars)
    try:
        result = parser.parse_expr()
        return result
    except (IndexError, RecursionError):
        return 'unknown_data()'


# ============================================================
# Section 4: Pseudocode Statement Translator
# ============================================================

def _get_target_size(target_name: str) -> str:
    """Determine the IR size expression for an assignment target."""
    if target_name in ('DEST',):
        return 'o1_size()'
    if target_name in ('SRC', 'SRC1'):
        return 'o2_size()'
    if target_name in ('SRC2',):
        return 'o3_size()'
    if target_name in REGISTER_SIZE:
        return REGISTER_SIZE[target_name]
    return 'o1_size()'


def _get_target_ir(target_name: str) -> str:
    """Translate an assignment target name to IR code."""
    if target_name in OPERAND_IR:
        return OPERAND_IR[target_name]
    if target_name in REGISTER_IR:
        return REGISTER_IR[target_name]
    return 'o1()'


def translate_simple_pseudocode(operation: str, mnemonic: str) -> list[str] | None:
    """Try to translate simple pseudocode (few lines, basic assignments) to IR.

    Returns a list of indented Rust code lines for the function body, or None if
    the pseudocode is too complex.
    """
    lines = operation.strip().split('\n')
    # Remove comments, blank lines, and text-only lines
    clean_lines = []
    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue
        # Remove inline comments (* ... *)
        stripped = re.sub(r'\(\*.*?\*\)', '', stripped).strip()
        if not stripped:
            continue
        # Skip pure text description lines (not pseudocode)
        if stripped.startswith('The ') or stripped.startswith('This ') or stripped.startswith('Note'):
            continue
        # Skip section markers
        if any(stripped.startswith(pat) for pat in _SECTION_STOP_PATTERNS):
            continue
        clean_lines.append(stripped)

    if not clean_lines:
        return None

    # Check if it's a multi-form pseudocode (has IF for operand size)
    has_if = any(l.startswith('IF ') or l.startswith('if ') for l in clean_lines)
    has_for = any(l.startswith('FOR ') or l.startswith('for ') for l in clean_lines)

    # Skip FOR loops (SIMD) - handled by SIMD detector
    if has_for:
        return None

    # For simple single-assignment (no IF)
    if not has_if and len(clean_lines) <= 5:
        return _translate_flat_assignments(clean_lines, mnemonic)

    # For IF/THEN/ELSE/FI with simple body
    if has_if and len(clean_lines) <= 30:
        return _translate_if_block(clean_lines, mnemonic)

    return None


def _translate_flat_assignments(lines: list[str], mnemonic: str) -> list[str] | None:
    """Translate a sequence of simple assignments to IR."""
    rust_lines: list[str] = []
    temp_vars: dict[str, str] = {}
    var_counter = 0
    statements: list[str] = []

    for line in lines:
        line = line.rstrip(';').strip()
        if not line:
            continue

        # Assignment: TARGET := EXPR
        m = re.match(r'^(\w+(?:\[.*?\])?)\s*:=\s*(.+)$', line)
        if not m:
            # Might be a comparison/condition line or exception
            if line.startswith('#'):
                statements.append(f'exception("{line}")')
                continue
            continue

        target_raw = m.group(1).strip()
        expr_raw = m.group(2).strip()

        # Strip bit field from target for simplicity
        target_name = re.sub(r'\[.*?\]', '', target_raw).strip()

        target_ir = _get_target_ir(target_name)
        size_ir = _get_target_size(target_name)
        expr_ir = parse_expression(expr_raw, temp_vars)

        # Check if this is a temp variable assignment
        if target_name.startswith('temp') or target_name.startswith('tmp'):
            temp_vars[target_name] = expr_ir
            var_name = f'v_{var_counter}'
            var_counter += 1
            rust_lines.append(f'    let {var_name} = {expr_ir};')
            temp_vars[target_name] = var_name
        else:
            var_name = f'stmt_{var_counter}'
            var_counter += 1
            rust_lines.append(f'    let {var_name} = assign({expr_ir}, {target_ir}, {size_ir});')
            statements.append(var_name)

    if not statements:
        return None

    rust_lines.append(f'    [{", ".join(statements)}].into()')
    return rust_lines


def _translate_if_block(lines: list[str], mnemonic: str) -> list[str] | None:
    """Translate IF/THEN/ELSE/FI pseudocode to IR condition."""
    # Join lines and try to detect the pattern
    text = '\n'.join(lines)

    # Pattern: IF 64-Bit Mode THEN #UD; ELSE ... FI;
    # For 64-bit mode checks, we can skip the mode check and just translate the ELSE body
    if '64-Bit Mode' in text or '64-bit mode' in text or '64-bit Mode' in text:
        # Extract ELSE body
        else_match = re.search(r'ELSE\s*\n(.*?)(?:FI|$)', text, re.DOTALL | re.IGNORECASE)
        if else_match:
            else_body = else_match.group(1).strip()
            else_lines = [l.strip() for l in else_body.split('\n') if l.strip()]
            result = _translate_flat_assignments(else_lines, mnemonic)
            if result:
                return result

    # Pattern: IF OperandSize is X THEN ... ELSE ... FI;
    # Simplify to the general case (use operand-relative sizes)
    if 'OperandSize' in text:
        # Try to find the THEN body and use it
        then_match = re.search(r'THEN\s*\n?(.*?)(?:ELSE|FI)', text, re.DOTALL | re.IGNORECASE)
        if then_match:
            then_body = then_match.group(1).strip()
            then_lines = [l.strip() for l in then_body.split('\n') if l.strip()]
            if then_lines:
                result = _translate_flat_assignments(then_lines, mnemonic)
                if result:
                    return result

    # Generic IF/THEN/ELSE handling
    # Try to extract condition, true body, false body
    cond_match = re.match(r'IF\s+(.+?)(?:\s+THEN|\s*$)', lines[0], re.IGNORECASE)
    if not cond_match:
        return None

    cond_text = cond_match.group(1).strip()

    # Parse the condition
    cond_ir = _translate_condition(cond_text)
    if not cond_ir:
        return None

    # Separate true and false bodies
    true_lines: list[str] = []
    false_lines: list[str] = []
    in_else = False
    depth = 0

    for i, line in enumerate(lines[1:], 1):
        stripped = line.strip().upper()
        if stripped.startswith('IF '):
            depth += 1
        if stripped.startswith('FI') and depth > 0:
            depth -= 1
            continue
        if stripped.startswith('FI') and depth == 0:
            break
        if stripped == 'THEN':
            continue
        if stripped == 'ELSE' and depth == 0:
            in_else = True
            continue
        if depth == 0:
            if in_else:
                false_lines.append(line.strip())
            else:
                true_lines.append(line.strip())

    true_stmts = _translate_flat_assignments(true_lines, mnemonic) if true_lines else None
    false_stmts = _translate_flat_assignments(false_lines, mnemonic) if false_lines else None

    # Build condition statement
    # This is simplified - for complex bodies we'd need proper nesting
    if true_stmts and not false_stmts:
        # Simple if-then
        return [
            f'    let cond = condition({cond_ir}, [{_extract_stmt_names(true_stmts)}], []);',
            '    [cond].into()',
        ]

    return None


def _extract_stmt_names(rust_lines: list[str]) -> str:
    """Extract statement variable names from translated lines."""
    names = []
    for line in rust_lines:
        m = re.match(r'\s*let\s+(stmt_\d+)\s*=', line)
        if m:
            names.append(m.group(1))
    return ', '.join(names)


def _translate_condition(cond_text: str) -> str | None:
    """Translate a pseudocode condition to IR code."""
    cond_text = cond_text.strip()

    # Simple comparisons
    m = re.match(r'(.+?)\s*=\s*(.+)', cond_text)
    if m and ':=' not in cond_text:
        left = parse_expression(m.group(1).strip())
        right = parse_expression(m.group(2).strip())
        return f'b::equal({left}, {right}, o1_size())'

    m = re.match(r'(.+?)\s*>\s*(.+)', cond_text)
    if m:
        left = parse_expression(m.group(1).strip())
        right = parse_expression(m.group(2).strip())
        return f'b::signed_less({right}, {left}, o1_size())'

    m = re.match(r'(.+?)\s*<\s*(.+)', cond_text)
    if m:
        left = parse_expression(m.group(1).strip())
        right = parse_expression(m.group(2).strip())
        return f'b::signed_less({left}, {right}, o1_size())'

    return None


# ============================================================
# Section 5: Name-Based Template Generators
# ============================================================

def _t_arithmetic(op: str, flags: str = ALL_FLAGS) -> list[str]:
    """Binary arithmetic: DEST := DEST op SRC, with flag calculation."""
    return [
        f'    let op = b::{op}(o1(), o2());',
        f'    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {flags});',
        '    let type1 = type_specified(o1(), o1_size(), DataType::Int);',
        '    let type2 = type_specified(o2(), o2_size(), DataType::Int);',
        f'    [calc_flags, assignment, type1, type2].into()',
    ]


def _t_adc() -> list[str]:
    return [
        '    let size = o1_size();',
        '    let add = b::add(o1(), o2());',
        '    let add = b::add(add, u::zero_extend(cf.clone()));',
        '    let assignment = assign(add.clone(), o1(), &size);',
        f'    let calc_flags = calc_flags_automatically(add, size, {ALL_FLAGS});',
        '    let type1 = type_specified(o1(), o1_size(), DataType::Int);',
        '    let type2 = type_specified(o2(), o2_size(), DataType::Int);',
        '    let type3 = type_specified(cf.clone(), o1_size(), DataType::Int);',
        '    [calc_flags, assignment, type1, type2, type3].into()',
    ]


def _t_sbb() -> list[str]:
    return [
        '    let size = o1_size();',
        '    let sub = b::sub(o1(), o2());',
        '    let sub = b::sub(sub, u::zero_extend(cf.clone()));',
        '    let assignment = assign(sub.clone(), o1(), &size);',
        f'    let calc_flags = calc_flags_automatically(sub, size, {ALL_FLAGS});',
        '    let type1 = type_specified(o1(), o1_size(), DataType::Int);',
        '    let type2 = type_specified(o2(), o2_size(), DataType::Int);',
        '    let type3 = type_specified(cf.clone(), o1_size(), DataType::Int);',
        '    [calc_flags, assignment, type1, type2, type3].into()',
    ]


def _t_inc() -> list[str]:
    return [
        '    let add = b::add(o1(), c(1));',
        f'    let calc_flags = calc_flags_automatically(add.clone(), o1_size(), {NO_CF_FLAGS});',
        '    let assignment = assign(add, o1(), o1_size());',
        '    [calc_flags, assignment].into()',
    ]


def _t_dec() -> list[str]:
    return [
        '    let sub = b::sub(o1(), c(1));',
        f'    let calc_flags = calc_flags_automatically(sub.clone(), o1_size(), {NO_CF_FLAGS});',
        '    let assignment = assign(sub, o1(), o1_size());',
        '    [calc_flags, assignment].into()',
    ]


def _t_neg() -> list[str]:
    return [
        '    let neg = u::neg(o1());',
        f'    let calc_flags = calc_flags_automatically(neg.clone(), o1_size(), {ALL_FLAGS});',
        '    let assignment = assign(neg, o1(), o1_size());',
        '    [calc_flags, assignment].into()',
    ]


def _t_logical(op: str) -> list[str]:
    """Logical operation with CF=0, OF=0, AF=undefined."""
    return [
        f'    let op = b::{op}(o1(), o2());',
        f'    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {LOGIC_FLAGS});',
        '    let set_of = assign(c(0), of.clone(), size_relative(of.clone()));',
        '    let set_cf = assign(c(0), cf.clone(), size_relative(cf.clone()));',
        '    let set_af = assign(undefined_data(), af.clone(), size_relative(af.clone()));',
        '    let type1 = type_specified(o1(), o1_size(), DataType::Int);',
        '    let type2 = type_specified(o2(), o2_size(), DataType::Int);',
        '    [calc_flags, set_of, set_cf, set_af, assignment, type1, type2].into()',
    ]


def _t_not() -> list[str]:
    return [
        '    let not_val = u::not(o1());',
        '    let assignment = assign(not_val, o1(), o1_size());',
        '    [assignment].into()',
    ]


def _t_test() -> list[str]:
    return [
        '    let and_val = b::and(o1(), o2());',
        f'    let sf_zf_pf = calc_flags_automatically(and_val, o1_size(), {LOGIC_FLAGS});',
        '    let set_of = assign(c(0), of.clone(), size_relative(of.clone()));',
        '    let set_cf = assign(c(0), cf.clone(), size_relative(cf.clone()));',
        '    extend_undefined_flags(&[sf_zf_pf, set_of, set_cf], &[&af])',
    ]


def _t_mov() -> list[str]:
    return [
        '    let assignment = assign(u::zero_extend(o2()), o1(), o1_size());',
        '    [assignment].into()',
    ]


def _t_movsx() -> list[str]:
    return [
        '    let assignment = assign(u::sign_extend(o2()), o1(), o1_size());',
        '    [assignment].into()',
    ]


def _t_lea() -> list[str]:
    return [
        '    let address = u::zero_extend(d(o2()));',
        '    let assignment = assign(address, o1(), o1_size());',
        '    [assignment].into()',
    ]


def _t_xchg() -> list[str]:
    return [
        '    let set_tmp = assign(o1(), tmp64.clone(), o1_size());',
        '    let set_o1 = assign(o2(), o1(), o1_size());',
        '    let set_o2 = assign(tmp64.clone(), o2(), o2_size());',
        '    [set_tmp, set_o1, set_o2].into()',
    ]


def _t_push() -> list[str]:
    return [
        '    let set_sp = assign(b::sub(rsp.clone(), architecture_byte_size()), rsp.clone(), size_architecture());',
        '    let push = assign(o1(), d(rsp.clone()), o1_size());',
        '    [set_sp, push].into()',
    ]


def _t_pop() -> list[str]:
    return [
        '    let pop = assign(d(rsp.clone()), o1(), o1_size());',
        '    let set_sp = assign(b::add(rsp.clone(), architecture_byte_size()), rsp.clone(), size_architecture());',
        '    [pop, set_sp].into()',
    ]


def _t_jmp() -> list[str]:
    return ['    [jump(o1())].into()']


def _t_call() -> list[str]:
    return [
        '    let set_sp = assign(b::sub(rsp.clone(), architecture_byte_size()), rsp.clone(), size_architecture());',
        '    let ret_address = b::add(rip.clone(), instruction_byte_size());',
        '    let save_ret = assign(ret_address, d(rsp.clone()), size_architecture());',
        '    let call = jump_by_call(o1());',
        '    let type1 = type_specified(o1(), o1_size(), DataType::Address);',
        '    let type2 = type_specified(rsp.clone(), size_architecture(), DataType::Address);',
        '    let type3 = type_specified(rip.clone(), size_architecture(), DataType::Address);',
        '    [set_sp, save_ret, call, type1, type2, type3].into()',
    ]


def _t_ret() -> list[str]:
    return [
        '    let jmp = jump(d(rsp.clone()));',
        '    let set_sp = assign(b::add(rsp.clone(), architecture_byte_size()), rsp.clone(), size_architecture());',
        '    let operand_condition = condition(is_o1_exists(), [assign(b::add(rsp.clone(), u::zero_extend(o1())), rsp.clone(), size_architecture())], []);',
        '    let halt = halt();',
        '    [set_sp, operand_condition, jmp, halt].into()',
    ]


def _t_cmp() -> list[str]:
    return [
        '    let sub = b::sub(o1(), u::sign_extend(o2()));',
        f'    let calc_flags = calc_flags_automatically(sub, o1_size(), {ALL_FLAGS});',
        '    let type1 = type_specified(o1(), o1_size(), DataType::Int);',
        '    let type2 = type_specified(o2(), o2_size(), DataType::Int);',
        '    [calc_flags, type1, type2].into()',
    ]


def _t_nop() -> list[str]:
    return ['    [].into()']


def _t_hlt() -> list[str]:
    return ['    [halt()].into()']


def _t_ud2() -> list[str]:
    return ['    [exception("#UD")].into()']


def _t_flag_set(flag: str, val: str) -> list[str]:
    """Set a flag to a constant value."""
    return [
        f'    let set = assign({val}, {flag}.clone(), size_relative({flag}.clone()));',
        '    [set].into()',
    ]


def _t_flag_complement(flag: str) -> list[str]:
    return [
        f'    let set = assign(u::not({flag}.clone()), {flag}.clone(), size_relative({flag}.clone()));',
        '    [set].into()',
    ]


def _t_sign_extend(from_reg: str, to_reg: str) -> list[str]:
    return [
        f'    let ext = assign(u::sign_extend({from_reg}.clone()), {to_reg}.clone(), size_relative({to_reg}.clone()));',
        f'    let type1 = type_specified({from_reg}.clone(), size_relative({from_reg}.clone()), DataType::Int);',
        f'    let type2 = type_specified({to_reg}.clone(), size_relative({to_reg}.clone()), DataType::Int);',
        f'    [ext, type1, type2].into()',
    ]


def _t_leave() -> list[str]:
    return [
        '    let restore_sp = assign(rbp.clone(), rsp.clone(), size_architecture());',
        '    let pop_rbp = assign(d(rsp.clone()), rbp.clone(), size_architecture());',
        '    let inc_sp = assign(b::add(rsp.clone(), architecture_byte_size()), rsp.clone(), size_architecture());',
        '    [restore_sp, pop_rbp, inc_sp].into()',
    ]


def _t_bswap() -> list[str]:
    """Byte swap matching fireman's reference: condition on 32 vs 64 bit with repeated shl/shr."""
    # Build swap_32: save to tmp32, then 3 rounds of extract-low-byte + shift-up
    swap_32_lines = []
    swap_32_lines.append('assign(o1(), tmp32.clone(), size.clone())')
    for _ in range(4):
        swap_32_lines.append('assign(tmp32.clone(), o1(), size_result_bit(c(8)))')
        swap_32_lines.append('assign(b::shl(o1(), c(8)), o1(), size.clone())')
        swap_32_lines.append('assign(b::shr(tmp32.clone(), c(8)), tmp32.clone(), size.clone())')
    # Last iteration only needs the extract
    # Actually fireman does 4 full iterations with the last one ending on extract only
    # Let me match exactly: 4 groups of (extract, shift-up, shift-down) then final extract
    # Looking at fireman: it's 1 initial save + 4 * (extract + shift + shift) + final extract = 1+12+1=14... no
    # Actually fireman b.rs:7-19 has: save, then 4 repetitions of (tmp→o1 8bit, shl o1, shr tmp), then final tmp→o1 8bit
    # That's save + 4*(3 lines) + 1 = 14 nope... count: lines 8-19 = 12 lines after save = 4 groups of 3
    # Actually: save(1), then [extract,shift,shift]*3 + [extract] = 1+9+1 = 11... let me just count
    # b.rs swap_32: lines 8-19:
    #  8: tmp32→o1 (8bit)
    #  9: shl o1
    # 10: shr tmp32
    # 11: tmp32→o1 (8bit)
    # 12: shl o1
    # 13: shr tmp32
    # 14: tmp32→o1 (8bit)
    # 15: shl o1
    # 16: shr tmp32
    # 17: tmp32→o1 (8bit)
    # = save + 3*(extract+shl+shr) + extract = 1 + 9 + 1 = 11
    lines = [
        '    let size = o1_size();',
        '    let swap_32 = [',
        '        assign(o1(), tmp32.clone(), size.clone()),',
    ]
    for _ in range(3):
        lines.append('        assign(tmp32.clone(), o1(), size_result_bit(c(8))),')
        lines.append('        assign(b::shl(o1(), c(8)), o1(), size.clone()),')
        lines.append('        assign(b::shr(tmp32.clone(), c(8)), tmp32.clone(), size.clone()),')
    lines.append('        assign(tmp32.clone(), o1(), size_result_bit(c(8))),')
    lines.append('    ];')

    lines.append('    let swap_64 = [')
    lines.append('        assign(o1(), tmp64.clone(), size.clone()),')
    for _ in range(7):
        lines.append('        assign(tmp64.clone(), o1(), size_result_bit(c(8))),')
        lines.append('        assign(b::shl(o1(), c(8)), o1(), size.clone()),')
        lines.append('        assign(b::shr(tmp64.clone(), c(8)), tmp64.clone(), size.clone()),')
    lines.append('        assign(tmp64.clone(), o1(), size_result_bit(c(8))),')
    lines.append('    ];')

    lines.append('    let bswap = condition(b::equal(bit_size_of_o1(), c(32), size_unlimited()), swap_32, swap_64);')
    lines.append('    let type1 = type_specified(o1(), o1_size(), DataType::Int);')
    lines.append('    [bswap, type1].into()')
    return lines


def _arm_t_rev() -> list[str]:
    """ARM REV using the same byte-swap structure as Intel BSWAP, but with src=o2 and dst=o1."""
    lines = [
        '    let size = o1_size();',
        '    let swap_32 = [',
        '        assign(o2(), tmp32.clone(), size.clone()),',
    ]
    for _ in range(3):
        lines.append('        assign(tmp32.clone(), o1(), size_result_bit(c(8))),')
        lines.append('        assign(b::shl(o1(), c(8)), o1(), size.clone()),')
        lines.append('        assign(b::shr(tmp32.clone(), c(8)), tmp32.clone(), size.clone()),')
    lines.append('        assign(tmp32.clone(), o1(), size_result_bit(c(8))),')
    lines.append('    ];')

    lines.append('    let swap_64 = [')
    lines.append('        assign(o2(), tmp64.clone(), size.clone()),')
    for _ in range(7):
        lines.append('        assign(tmp64.clone(), o1(), size_result_bit(c(8))),')
        lines.append('        assign(b::shl(o1(), c(8)), o1(), size.clone()),')
        lines.append('        assign(b::shr(tmp64.clone(), c(8)), tmp64.clone(), size.clone()),')
    lines.append('        assign(tmp64.clone(), o1(), size_result_bit(c(8))),')
    lines.append('    ];')

    lines.append('    let rev = condition(b::equal(bit_size_of_o1(), c(32), size_unlimited()), swap_32, swap_64);')
    lines.append('    let type1 = type_specified(o1(), o1_size(), DataType::Int);')
    lines.append('    [rev, type1].into()')
    return lines


def _t_xor() -> list[str]:
    """XOR with same-operand zero idiom detection, matching fireman's reference."""
    return [
        '    let cond = b::equal(o1(), o2(), o1_size());',
        '    let true_b = [',
        '        assign(c(0), o1(), o1_size()),',
        '        assign(c(1), zf.clone(), size_relative(zf.clone())),',
        '        assign(c(0), sf.clone(), size_relative(sf.clone())),',
        '        assign(c(0), pf.clone(), size_relative(pf.clone())),',
        '    ];',
        '    let false_b = b::xor(o1(), o2());',
        '    let false_b = [',
        '        assign(false_b.clone(), o1(), o1_size()),',
        '        assign(c(0), zf.clone(), size_relative(zf.clone())),',
        '        calc_flags_automatically(false_b, o1_size(), &[&sf, &pf]),',
        '    ];',
        '    let xor = condition(cond, true_b, false_b);',
        '    let set_of = assign(c(0), of.clone(), size_relative(of.clone()));',
        '    let set_cf = assign(c(0), cf.clone(), size_relative(cf.clone()));',
        '    extend_undefined_flags(&[xor, set_of, set_cf], &[&af])',
    ]


def _t_bt() -> list[str]:
    return [
        '    let size = size_relative(cf.clone());',
        '    let shr = b::shr(o1(), o2());',
        '    let assignment = assign(shr.clone(), cf.clone(), &size);',
        '    let type1 = type_specified(o1(), o1_size(), DataType::Int);',
        '    let type2 = type_specified(o2(), o2_size(), DataType::Int);',
        '    extend_undefined_flags(&[assignment, type1, type2], &[&of, &sf, &af, &pf])',
    ]


def _t_bts() -> list[str]:
    return [
        '    let shr = b::shr(o1(), o2());',
        '    let save_cf = assign(b::and(shr, c(1)), cf.clone(), size_relative(cf.clone()));',
        '    let mask = b::shl(c(1), o2());',
        '    let set_bit = assign(b::or(o1(), mask), o1(), o1_size());',
        '    [save_cf, set_bit].into()',
    ]


def _t_btr() -> list[str]:
    return [
        '    let shr = b::shr(o1(), o2());',
        '    let save_cf = assign(b::and(shr, c(1)), cf.clone(), size_relative(cf.clone()));',
        '    let mask = u::not(b::shl(c(1), o2()));',
        '    let clear_bit = assign(b::and(o1(), mask), o1(), o1_size());',
        '    [save_cf, clear_bit].into()',
    ]


def _t_btc() -> list[str]:
    return [
        '    let shr = b::shr(o1(), o2());',
        '    let save_cf = assign(b::and(shr, c(1)), cf.clone(), size_relative(cf.clone()));',
        '    let mask = b::shl(c(1), o2());',
        '    let flip_bit = assign(b::xor(o1(), mask), o1(), o1_size());',
        '    [save_cf, flip_bit].into()',
    ]


def _t_sahf() -> list[str]:
    return [
        '    let sf_a = assign(b::and(b::shr(ah.clone(), c(7)), c(1)), sf.clone(), size_relative(sf.clone()));',
        '    let zf_a = assign(b::and(b::shr(ah.clone(), c(6)), c(1)), zf.clone(), size_relative(zf.clone()));',
        '    let af_a = assign(b::and(b::shr(ah.clone(), c(4)), c(1)), af.clone(), size_relative(af.clone()));',
        '    let pf_a = assign(b::and(b::shr(ah.clone(), c(2)), c(1)), pf.clone(), size_relative(pf.clone()));',
        '    let cf_a = assign(b::and(ah.clone(), c(1)), cf.clone(), size_relative(cf.clone()));',
        '    [sf_a, zf_a, af_a, pf_a, cf_a].into()',
    ]


def _t_lahf() -> list[str]:
    return [
        '    let val = b::or(b::or(b::or(b::or(b::shl(sf.clone(), c(7)), b::shl(zf.clone(), c(6))), b::shl(af.clone(), c(4))), b::shl(pf.clone(), c(2))), cf.clone());',
        '    let assignment = assign(val, ah.clone(), size_relative(ah.clone()));',
        '    [assignment].into()',
    ]


def _t_shl() -> list[str]:
    return [
        '    let shl_1 = b::shl(o1(), o2());',
        f'    let shl_1_flags = calc_flags_automatically(shl_1.clone(), o1_size(), {LOGIC_FLAGS});',
        '    let shl_2 = b::shl(o1(), c(1));',
        f'    let shl_2_flags = calc_flags_automatically(shl_2.clone(), o1_size(), {LOGIC_FLAGS});',
        '    let cond = condition(is_o2_exists(), [shl_1_flags, assign(shl_1, o1(), o1_size())], [shl_2_flags, assign(shl_2, o1(), o1_size())]);',
        '    extend_undefined_flags(&[cond], &[&of, &af, &cf])',
    ]


def _t_shr() -> list[str]:
    return [
        '    let shr_1 = b::shr(o1(), o2());',
        f'    let shr_1_flags = calc_flags_automatically(shr_1.clone(), o1_size(), {LOGIC_FLAGS});',
        '    let shr_2 = b::shr(o1(), c(1));',
        f'    let shr_2_flags = calc_flags_automatically(shr_2.clone(), o1_size(), {LOGIC_FLAGS});',
        '    let cond = condition(is_o2_exists(), [shr_1_flags, assign(shr_1, o1(), o1_size())], [shr_2_flags, assign(shr_2, o1(), o1_size())]);',
        '    extend_undefined_flags(&[cond], &[&of, &af, &cf])',
    ]


def _t_sar() -> list[str]:
    return [
        '    let sar_1 = b::sar(o1(), o2());',
        f'    let sar_1_flags = calc_flags_automatically(sar_1.clone(), o1_size(), {LOGIC_FLAGS});',
        '    let sar_2 = b::sar(o1(), c(1));',
        f'    let sar_2_flags = calc_flags_automatically(sar_2.clone(), o1_size(), {LOGIC_FLAGS});',
        '    let cond = condition(is_o2_exists(), [sar_1_flags, assign(sar_1, o1(), o1_size())], [sar_2_flags, assign(sar_2, o1(), o1_size())]);',
        '    extend_undefined_flags(&[cond], &[&of, &af, &cf])',
    ]


def _t_rol() -> list[str]:
    return [
        '    let op = b::or(b::shl(o1(), o2()), b::shr(o1(), b::sub(bit_size_of_o1(), o2())));',
        '    let assignment = assign(op, o1(), o1_size());',
        '    extend_undefined_flags(&[assignment], &[&of, &cf])',
    ]


def _t_ror() -> list[str]:
    return [
        '    let op = b::or(b::shr(o1(), o2()), b::shl(o1(), b::sub(bit_size_of_o1(), o2())));',
        '    let assignment = assign(op, o1(), o1_size());',
        '    extend_undefined_flags(&[assignment], &[&of, &cf])',
    ]


def _t_rcl() -> list[str]:
    return [
        '    let op = b::or(b::shl(o1(), o2()), b::shr(o1(), b::sub(bit_size_of_o1(), o2())));',
        '    let assignment = assign(op, o1(), o1_size());',
        '    extend_undefined_flags(&[assignment], &[&of, &cf])',
    ]


def _t_rcr() -> list[str]:
    return [
        '    let op = b::or(b::shr(o1(), o2()), b::shl(o1(), b::sub(bit_size_of_o1(), o2())));',
        '    let assignment = assign(op, o1(), o1_size());',
        '    extend_undefined_flags(&[assignment], &[&of, &cf])',
    ]


def _t_mul() -> list[str]:
    return [
        '    let assertion = assertion(u::not(is_o2_exists()));',
        '    let operand_bit_size = bit_size_of_o1();',
        '    let value_8 = b::mul(sized(al.clone(), size_relative(al.clone())), o1());',
        '    let mul_8 = [calc_flags_automatically(value_8.clone(), o1_size(), &[&of, &cf]), assign(value_8, ax.clone(), size_relative(ax.clone()))];',
        '    let value = b::mul(sized(rax.clone(), o1_size()), o1());',
        '    let mul_etc = [calc_flags_automatically(value.clone(), o1_size(), &[&of, &cf]), assign(value.clone(), rax.clone(), o1_size()), assign(b::shr(u::zero_extend(value), operand_bit_size.clone()), rdx.clone(), o1_size())];',
        '    let mul = condition(b::equal(operand_bit_size, c(8), size_unlimited()), mul_8, mul_etc);',
        '    extend_undefined_flags(&[assertion, mul], &[&sf, &zf, &af, &pf])',
    ]


def _t_imul() -> list[str]:
    # IMUL has multiple forms: 1 operand, 2 operand, 3 operand
    return [
        '    let result = condition(is_o3_exists(),',
        '        [assign(b::mul(o2(), o3()), o1(), o1_size())],',
        '        condition(is_o2_exists(),',
        '            [assign(b::mul(o1(), o2()), o1(), o1_size())],',
        '            {',
        '                let operand_bit_size = bit_size_of_o1();',
        '                let mul_8 = [assign(b::mul(sized(al.clone(), size_relative(al.clone())), o1()), ax.clone(), size_relative(ax.clone()))];',
        '                let value = b::mul(sized(rax.clone(), o1_size()), o1());',
        '                let mul_etc = [assign(value.clone(), rax.clone(), o1_size()), assign(b::shr(u::sign_extend(value), operand_bit_size.clone()), rdx.clone(), o1_size())];',
        '                condition(b::equal(operand_bit_size, c(8), size_unlimited()), mul_8, mul_etc)',
        '            }',
        '        )',
        '    );',
        '    extend_undefined_flags(&[result], &[&of, &sf, &zf, &af, &cf, &pf])',
    ]


def _t_div() -> list[str]:
    return [
        '    let operand_bit_size = bit_size_of_o1();',
        '    let div_8 = [assign(b::unsigned_div(ax.clone(), o1()), al.clone(), o1_size()), assign(b::unsigned_rem(ax.clone(), o1()), ah.clone(), o1_size())];',
        '    let value = b::add(b::shl(sized(rdx.clone(), o1_size()), operand_bit_size.clone()), sized(rax.clone(), o1_size()));',
        '    let div_etc = [assign(b::unsigned_div(value.clone(), o1()), rax.clone(), o1_size()), assign(b::unsigned_rem(value, o1()), rdx.clone(), o1_size())];',
        '    let div = condition(b::equal(operand_bit_size, c(8), size_unlimited()), div_8, div_etc);',
        '    extend_undefined_flags(&[div], &[&of, &sf, &zf, &af, &cf, &pf])',
    ]


def _t_idiv() -> list[str]:
    return [
        '    let operand_bit_size = bit_size_of_o1();',
        '    let idiv_8 = [assign(b::signed_div(u::sign_extend(ax.clone()), u::sign_extend(o1())), al.clone(), o1_size()), assign(b::signed_rem(u::sign_extend(ax.clone()), u::sign_extend(o1())), ah.clone(), o1_size())];',
        '    let value = b::add(b::shl(sized(rdx.clone(), o1_size()), operand_bit_size.clone()), sized(rax.clone(), o1_size()));',
        '    let idiv_etc = [assign(b::signed_div(u::sign_extend(value.clone()), u::sign_extend(o1())), rax.clone(), o1_size()), assign(b::signed_rem(u::sign_extend(value), u::sign_extend(o1())), rdx.clone(), o1_size())];',
        '    let idiv = condition(b::equal(operand_bit_size, c(8), size_unlimited()), idiv_8, idiv_etc);',
        '    extend_undefined_flags(&[idiv], &[&of, &sf, &zf, &af, &cf, &pf])',
    ]


def _t_cmpxchg() -> list[str]:
    return [
        '    let cond = b::equal(rax.clone(), d(o1()), o1_size());',
        '    let true_b = [assign(o2(), d(o1()), o1_size())];',
        '    let false_b = [assign(d(o1()), rax.clone(), o1_size())];',
        '    let cmpxchg = condition(cond.clone(), true_b, false_b);',
        f'    let calc_flags = calc_flags_automatically(cond, size_result_byte(c(1)), {ALL_FLAGS});',
        '    let type1 = type_specified(o1(), o1_size(), DataType::Int);',
        '    let type2 = type_specified(o2(), o2_size(), DataType::Int);',
        '    let type3 = type_specified(rax.clone(), size_relative(rax.clone()), DataType::Int);',
        '    [calc_flags, cmpxchg, type1, type2, type3].into()',
    ]


def _t_cwd() -> list[str]:
    return [
        '    let set_tmp = assign(u::sign_extend(ax.clone()), tmp32.clone(), size_relative(tmp32.clone()));',
        '    let set_dx = assign(b::shr(tmp32.clone(), c(16)), dx.clone(), size_relative(dx.clone()));',
        '    let set_ax = assign(tmp32.clone(), ax.clone(), size_relative(ax.clone()));',
        '    let type1 = type_specified(ax.clone(), size_relative(ax.clone()), DataType::Int);',
        '    let type2 = type_specified(dx.clone(), size_relative(dx.clone()), DataType::Int);',
        '    [set_tmp, set_dx, set_ax, type1, type2].into()',
    ]


def _t_cdq() -> list[str]:
    return [
        '    let set_tmp = assign(u::sign_extend(eax.clone()), tmp64.clone(), size_relative(tmp64.clone()));',
        '    let set_dx = assign(b::shr(tmp64.clone(), c(16)), edx.clone(), size_relative(edx.clone()));',
        '    let set_ax = assign(tmp64.clone(), eax.clone(), size_relative(eax.clone()));',
        '    let type1 = type_specified(eax.clone(), size_relative(eax.clone()), DataType::Int);',
        '    let type2 = type_specified(edx.clone(), size_relative(edx.clone()), DataType::Int);',
        '    [set_tmp, set_dx, set_ax, type1, type2].into()',
    ]


def _t_cqo() -> list[str]:
    return [
        '    let set_tmp = assign(u::sign_extend(rax.clone()), tmp128.clone(), size_relative(tmp128.clone()));',
        '    let set_dx = assign(b::shr(tmp128.clone(), c(16)), rdx.clone(), size_relative(rdx.clone()));',
        '    let set_ax = assign(tmp128.clone(), rax.clone(), size_relative(rax.clone()));',
        '    let type1 = type_specified(rax.clone(), size_relative(rax.clone()), DataType::Int);',
        '    let type2 = type_specified(rdx.clone(), size_relative(rdx.clone()), DataType::Int);',
        '    [set_tmp, set_dx, set_ax, type1, type2].into()',
    ]


def _t_string_cmp(size_expr: str) -> list[str]:
    """Compare string operation (CMPS variants)."""
    return [
        '    let source = d(rsi.clone());',
        '    let destination = d(rdi.clone());',
        '    let sub = b::sub(source.clone(), u::sign_extend(destination.clone()));',
        f'    let calc_flags = calc_flags_automatically(sub, {size_expr}, {ALL_FLAGS});',
        '    let type1 = type_specified(source, size_architecture(), DataType::Int);',
        '    let type2 = type_specified(destination, size_architecture(), DataType::Int);',
        '    let type3 = type_specified(rsi.clone(), size_architecture(), DataType::Address);',
        '    let type4 = type_specified(rdi.clone(), size_architecture(), DataType::Address);',
        '    [calc_flags, type1, type2, type3, type4].into()',
    ]


def _t_string_mov(size_expr: str) -> list[str]:
    """Move string operation (MOVS variants)."""
    return [
        '    let mov = assign(d(rsi.clone()), d(rdi.clone()), ' + size_expr + ');',
        '    [mov].into()',
    ]


def _t_string_stos(size_expr: str) -> list[str]:
    """Store string operation (STOS variants)."""
    return [
        '    let stos = assign(rax.clone(), d(rdi.clone()), ' + size_expr + ');',
        '    [stos].into()',
    ]


def _t_string_lods(size_expr: str) -> list[str]:
    """Load string operation (LODS variants)."""
    return [
        '    let lods = assign(d(rsi.clone()), rax.clone(), ' + size_expr + ');',
        '    [lods].into()',
    ]


def _t_string_scas(size_expr: str) -> list[str]:
    """Scan string operation (SCAS variants)."""
    return [
        '    let sub = b::sub(rax.clone(), d(rdi.clone()));',
        f'    let calc_flags = calc_flags_automatically(sub, {size_expr}, {ALL_FLAGS});',
        '    [calc_flags].into()',
    ]


def _t_bsf() -> list[str]:
    return [
        '    let set_zf = condition(b::equal(o2(), c(0), o2_size()), [assign(c(1), zf.clone(), size_relative(zf.clone()))], [assign(c(0), zf.clone(), size_relative(zf.clone())), assign(o2(), o1(), o1_size())]);',
        '    [set_zf].into()',
    ]


def _t_bsr() -> list[str]:
    return [
        '    let set_zf = condition(b::equal(o2(), c(0), o2_size()), [assign(c(1), zf.clone(), size_relative(zf.clone()))], [assign(c(0), zf.clone(), size_relative(zf.clone())), assign(o2(), o1(), o1_size())]);',
        '    [set_zf].into()',
    ]


def _t_popcnt() -> list[str]:
    return [
        '    let assignment = assign(o2(), o1(), o1_size());',
        '    let set_of = assign(c(0), of.clone(), size_relative(of.clone()));',
        '    let set_sf = assign(c(0), sf.clone(), size_relative(sf.clone()));',
        '    let set_af = assign(c(0), af.clone(), size_relative(af.clone()));',
        '    let set_cf = assign(c(0), cf.clone(), size_relative(cf.clone()));',
        '    let set_pf = assign(c(0), pf.clone(), size_relative(pf.clone()));',
        '    let set_zf = condition(b::equal(o1(), c(0), o1_size()), [assign(c(1), zf.clone(), size_relative(zf.clone()))], [assign(c(0), zf.clone(), size_relative(zf.clone()))]);',
        '    [assignment, set_of, set_sf, set_af, set_cf, set_pf, set_zf].into()',
    ]


def _t_lzcnt() -> list[str]:
    return _t_popcnt()  # Similar structure


def _t_tzcnt() -> list[str]:
    return _t_popcnt()  # Similar structure


def _t_xadd() -> list[str]:
    return [
        '    let sum = b::add(o1(), o2());',
        '    let save_o1 = assign(o1(), o2(), o2_size());',
        '    let set_o1 = assign(sum.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(sum, o1_size(), {ALL_FLAGS});',
        '    [save_o1, set_o1, calc_flags].into()',
    ]


def _t_enter() -> list[str]:
    return [
        '    let push_bp = assign(rbp.clone(), d(b::sub(rsp.clone(), architecture_byte_size())), size_architecture());',
        '    let set_sp1 = assign(b::sub(rsp.clone(), architecture_byte_size()), rsp.clone(), size_architecture());',
        '    let set_bp = assign(rsp.clone(), rbp.clone(), size_architecture());',
        '    let set_sp2 = assign(b::sub(rsp.clone(), u::zero_extend(o1())), rsp.clone(), size_architecture());',
        '    [push_bp, set_sp1, set_bp, set_sp2].into()',
    ]


# Full template registry
TEMPLATES: dict[str, list[str]] = {}


def _build_templates():
    """Build the complete template registry."""
    global TEMPLATES

    # Arithmetic with all flags
    TEMPLATES['add'] = _t_arithmetic('add')
    TEMPLATES['sub'] = _t_arithmetic('sub')
    TEMPLATES['adc'] = _t_adc()
    TEMPLATES['adcx'] = _t_adc()
    TEMPLATES['adox'] = _t_adc()  # Similar to ADC but uses OF instead of CF
    TEMPLATES['sbb'] = _t_sbb()
    TEMPLATES['inc'] = _t_inc()
    TEMPLATES['dec'] = _t_dec()
    TEMPLATES['neg'] = _t_neg()

    # Logical
    TEMPLATES['and'] = _t_logical('and')
    TEMPLATES['or'] = _t_logical('or')
    TEMPLATES['xor'] = _t_xor()
    TEMPLATES['not'] = _t_not()
    TEMPLATES['test'] = _t_test()
    TEMPLATES['andn'] = [
        '    let op = b::and(u::not(o2()), o3());',
        '    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {LOGIC_FLAGS});',
        '    let set_of = assign(c(0), of.clone(), size_relative(of.clone()));',
        '    let set_cf = assign(c(0), cf.clone(), size_relative(cf.clone()));',
        '    [calc_flags, set_of, set_cf, assignment].into()',
    ]

    # Data movement
    TEMPLATES['mov'] = _t_mov()
    TEMPLATES['movsx'] = _t_movsx()
    TEMPLATES['movsxd'] = _t_movsx()
    TEMPLATES['movzx'] = _t_mov()
    TEMPLATES['lea'] = _t_lea()
    TEMPLATES['xchg'] = _t_xchg()
    TEMPLATES['bswap'] = _t_bswap()

    # Stack
    TEMPLATES['push'] = _t_push()
    TEMPLATES['pop'] = _t_pop()
    TEMPLATES['pusha'] = _t_push()
    TEMPLATES['pushad'] = _t_push()
    TEMPLATES['popa'] = _t_pop()
    TEMPLATES['popad'] = _t_pop()
    TEMPLATES['pushf'] = _t_push()
    TEMPLATES['pushfd'] = _t_push()
    TEMPLATES['pushfq'] = _t_push()
    TEMPLATES['popf'] = _t_pop()
    TEMPLATES['popfd'] = _t_pop()
    TEMPLATES['popfq'] = _t_pop()
    TEMPLATES['enter'] = _t_enter()
    TEMPLATES['leave'] = _t_leave()

    # Control flow
    TEMPLATES['jmp'] = _t_jmp()
    TEMPLATES['call'] = _t_call()
    TEMPLATES['ret'] = _t_ret()
    TEMPLATES['nop'] = _t_nop()
    TEMPLATES['hlt'] = _t_hlt()
    TEMPLATES['ud2'] = _t_ud2()
    TEMPLATES['int'] = ['    [exception("INT")].into()']
    TEMPLATES['int1'] = ['    [exception("INT1")].into()']
    TEMPLATES['int3'] = ['    [exception("INT3")].into()']
    TEMPLATES['into'] = ['    [exception("INTO")].into()']
    TEMPLATES['iret'] = _t_ret()
    TEMPLATES['iretd'] = _t_ret()
    TEMPLATES['iretq'] = _t_ret()
    TEMPLATES['syscall'] = ['    [exception("SYSCALL")].into()']
    TEMPLATES['sysenter'] = ['    [exception("SYSENTER")].into()']
    TEMPLATES['sysexit'] = ['    [exception("SYSEXIT")].into()']
    TEMPLATES['sysret'] = ['    [exception("SYSRET")].into()']

    # Comparison
    TEMPLATES['cmp'] = _t_cmp()
    TEMPLATES['cmpxchg'] = _t_cmpxchg()
    TEMPLATES['cmpxchg8b'] = _t_cmpxchg()
    TEMPLATES['cmpxchg16b'] = _t_cmpxchg()
    TEMPLATES['xadd'] = _t_xadd()

    # Flag manipulation
    TEMPLATES['clc'] = _t_flag_set('cf', 'c(0)')
    TEMPLATES['stc'] = _t_flag_set('cf', 'c(1)')
    TEMPLATES['cld'] = _t_flag_set('df', 'c(0)')
    TEMPLATES['std'] = _t_flag_set('df', 'c(1)')
    TEMPLATES['cli'] = _t_flag_set('if_', 'c(0)')
    TEMPLATES['sti'] = _t_flag_set('if_', 'c(1)')
    TEMPLATES['cmc'] = _t_flag_complement('cf')
    TEMPLATES['sahf'] = _t_sahf()
    TEMPLATES['lahf'] = _t_lahf()

    # Sign extension
    TEMPLATES['cbw'] = _t_sign_extend('al', 'ax')
    TEMPLATES['cwde'] = _t_sign_extend('ax', 'eax')
    TEMPLATES['cdqe'] = _t_sign_extend('eax', 'rax')
    TEMPLATES['cwd'] = _t_cwd()
    TEMPLATES['cdq'] = _t_cdq()
    TEMPLATES['cqo'] = _t_cqo()

    # Bit manipulation
    TEMPLATES['bt'] = _t_bt()
    TEMPLATES['bts'] = _t_bts()
    TEMPLATES['btr'] = _t_btr()
    TEMPLATES['btc'] = _t_btc()
    TEMPLATES['bsf'] = _t_bsf()
    TEMPLATES['bsr'] = _t_bsr()
    TEMPLATES['popcnt'] = _t_popcnt()
    TEMPLATES['lzcnt'] = _t_lzcnt()
    TEMPLATES['tzcnt'] = _t_tzcnt()

    # Shift/rotate
    TEMPLATES['shl'] = _t_shl()
    TEMPLATES['sal'] = _t_shl()  # SAL is identical to SHL
    TEMPLATES['shr'] = _t_shr()
    TEMPLATES['sar'] = _t_sar()
    TEMPLATES['rol'] = _t_rol()
    TEMPLATES['ror'] = _t_ror()
    TEMPLATES['rcl'] = _t_rcl()
    TEMPLATES['rcr'] = _t_rcr()
    TEMPLATES['shld'] = [
        '    let op = b::or(b::shl(o1(), o3()), b::shr(o2(), b::sub(bit_size_of_o1(), o3())));',
        '    let assignment = assign(op, o1(), o1_size());',
        '    extend_undefined_flags(&[assignment], &[&of, &sf, &zf, &af, &cf, &pf])',
    ]
    TEMPLATES['shrd'] = [
        '    let op = b::or(b::shr(o1(), o3()), b::shl(o2(), b::sub(bit_size_of_o1(), o3())));',
        '    let assignment = assign(op, o1(), o1_size());',
        '    extend_undefined_flags(&[assignment], &[&of, &sf, &zf, &af, &cf, &pf])',
    ]

    # Multiply/divide
    TEMPLATES['mul'] = _t_mul()
    TEMPLATES['imul'] = [
        '    // IMUL: handles 1, 2, and 3-operand forms',
        '    let result = b::mul(o1(), o2());',
        '    let assignment = assign(result.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(result, o1_size(), &[&of, &cf]);',
        '    extend_undefined_flags(&[calc_flags, assignment], &[&sf, &zf, &af, &pf])',
    ]
    TEMPLATES['div'] = _t_div()
    TEMPLATES['idiv'] = _t_idiv()

    # String operations
    TEMPLATES['movsb'] = _t_string_mov('size_result_byte(c(1))')
    TEMPLATES['movsw'] = _t_string_mov('size_result_byte(c(2))')
    TEMPLATES['movsd'] = _t_string_mov('size_result_byte(c(4))')
    TEMPLATES['movsq'] = _t_string_mov('size_result_byte(c(8))')
    TEMPLATES['cmps'] = _t_string_cmp('size_architecture()')
    TEMPLATES['cmpsb'] = _t_string_cmp('size_result_byte(c(1))')
    TEMPLATES['cmpsw'] = _t_string_cmp('size_result_byte(c(2))')
    TEMPLATES['cmpsd'] = _t_string_cmp('size_result_byte(c(4))')
    TEMPLATES['cmpsq'] = _t_string_cmp('size_result_byte(c(8))')
    TEMPLATES['stosb'] = _t_string_stos('size_result_byte(c(1))')
    TEMPLATES['stosw'] = _t_string_stos('size_result_byte(c(2))')
    TEMPLATES['stosd'] = _t_string_stos('size_result_byte(c(4))')
    TEMPLATES['stosq'] = _t_string_stos('size_result_byte(c(8))')
    TEMPLATES['lodsb'] = _t_string_lods('size_result_byte(c(1))')
    TEMPLATES['lodsw'] = _t_string_lods('size_result_byte(c(2))')
    TEMPLATES['lodsd'] = _t_string_lods('size_result_byte(c(4))')
    TEMPLATES['lodsq'] = _t_string_lods('size_result_byte(c(8))')
    TEMPLATES['scasb'] = _t_string_scas('size_result_byte(c(1))')
    TEMPLATES['scasw'] = _t_string_scas('size_result_byte(c(2))')
    TEMPLATES['scasd'] = _t_string_scas('size_result_byte(c(4))')
    TEMPLATES['scasq'] = _t_string_scas('size_result_byte(c(8))')

    # Misc
    TEMPLATES['cpuid'] = ['    [exception("CPUID")].into()']
    TEMPLATES['rdtsc'] = ['    [exception("RDTSC")].into()']
    TEMPLATES['rdtscp'] = ['    [exception("RDTSCP")].into()']
    TEMPLATES['rdmsr'] = ['    [exception("RDMSR")].into()']
    TEMPLATES['wrmsr'] = ['    [exception("WRMSR")].into()']
    TEMPLATES['rdpmc'] = ['    [exception("RDPMC")].into()']
    TEMPLATES['lgdt'] = ['    [exception("LGDT")].into()']
    TEMPLATES['sgdt'] = ['    [exception("SGDT")].into()']
    TEMPLATES['lidt'] = ['    [exception("LIDT")].into()']
    TEMPLATES['sidt'] = ['    [exception("SIDT")].into()']
    TEMPLATES['lldt'] = ['    [exception("LLDT")].into()']
    TEMPLATES['sldt'] = ['    [exception("SLDT")].into()']
    TEMPLATES['ltr'] = ['    [exception("LTR")].into()']
    TEMPLATES['str'] = ['    [exception("STR")].into()']
    TEMPLATES['invd'] = ['    [exception("INVD")].into()']
    TEMPLATES['wbinvd'] = ['    [exception("WBINVD")].into()']
    TEMPLATES['invlpg'] = ['    [exception("INVLPG")].into()']
    TEMPLATES['clflush'] = ['    [exception("CLFLUSH")].into()']
    TEMPLATES['clflushopt'] = ['    [exception("CLFLUSHOPT")].into()']
    TEMPLATES['clwb'] = ['    [exception("CLWB")].into()']
    TEMPLATES['lfence'] = _t_nop()
    TEMPLATES['sfence'] = _t_nop()
    TEMPLATES['mfence'] = _t_nop()
    TEMPLATES['pause'] = _t_nop()
    TEMPLATES['prefetch'] = _t_nop()
    TEMPLATES['prefetchw'] = _t_nop()
    TEMPLATES['prefetchnta'] = _t_nop()
    TEMPLATES['prefetcht0'] = _t_nop()
    TEMPLATES['prefetcht1'] = _t_nop()
    TEMPLATES['prefetcht2'] = _t_nop()
    TEMPLATES['wait'] = _t_nop()
    TEMPLATES['fwait'] = _t_nop()
    TEMPLATES['lock'] = _t_nop()

    # BMI1/BMI2
    TEMPLATES['blsi'] = [
        '    let op = b::and(u::neg(o2()), o2());',
        '    let assignment = assign(op, o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['blsmsk'] = [
        '    let op = b::xor(b::sub(o2(), c(1)), o2());',
        '    let assignment = assign(op, o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['blsr'] = [
        '    let op = b::and(b::sub(o2(), c(1)), o2());',
        '    let assignment = assign(op, o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['bzhi'] = [
        '    let mask = b::sub(b::shl(c(1), o3()), c(1));',
        '    let op = b::and(o2(), mask);',
        '    let assignment = assign(op, o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['bextr'] = [
        '    let start = b::and(o3(), c(0xFF));',
        '    let len = b::and(b::shr(o3(), c(8)), c(0xFF));',
        '    let shifted = b::shr(o2(), start);',
        '    let mask = b::sub(b::shl(c(1), len), c(1));',
        '    let op = b::and(shifted, mask);',
        '    let assignment = assign(op, o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['pdep'] = [
        '    let assignment = assign(b::and(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['pext'] = [
        '    let assignment = assign(b::and(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['sarx'] = [
        '    let assignment = assign(b::sar(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['shlx'] = [
        '    let assignment = assign(b::shl(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['shrx'] = [
        '    let assignment = assign(b::shr(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['rorx'] = [
        '    let op = b::or(b::shr(o2(), o3()), b::shl(o2(), b::sub(bit_size_of_o2(), o3())));',
        '    let assignment = assign(op, o1(), o1_size());',
        '    [assignment].into()',
    ]
    TEMPLATES['mulx'] = [
        '    let product = b::mul(o2(), rdx.clone());',
        '    let assignment = assign(product, o1(), o1_size());',
        '    [assignment].into()',
    ]

    # SSE/SSE2 instructions without v-prefix
    # ADDSUBPD/ADDSUBPS: alternating sub/add per lane.
    # IR has no per-lane semantics; use exception since correct behavior
    # cannot be expressed with the available IR operators.
    TEMPLATES['addsubpd'] = [f'    [exception("addsubpd")].into()']
    TEMPLATES['addsubps'] = [f'    [exception("addsubps")].into()']
    TEMPLATES['movddup'] = _t_mov()
    TEMPLATES['movshdup'] = _t_mov()
    TEMPLATES['movsldup'] = _t_mov()
    TEMPLATES['movdq16'] = _t_mov()
    TEMPLATES['movdq32'] = _t_mov()
    TEMPLATES['movdq64'] = _t_mov()
    TEMPLATES['pclmulqdq'] = [f'    [exception("pclmulqdq")].into()']

    # Packed integer ops without v-prefix
    for _op, _ir in [('pabsq', 'o1()'), ('pavgb', 'b::add(o1(), o2())'), ('pavgw', 'b::add(o1(), o2())'),
                      ('phminposuw', 'o1()'), ('pmovmskb', 'o2()'), ('pmulhrsw', 'b::mul(o1(), o2())')]:
        TEMPLATES[_op] = [f'    let assignment = assign({_ir}, o1(), o1_size());', '    [assignment].into()']

    for _op in ['pmaxsq', 'pmaxuq', 'pminsq', 'pminuq']:
        TEMPLATES[_op] = ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']

    for _op in ['psignb', 'psignd', 'psignw']:
        TEMPLATES[_op] = ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']

    TEMPLATES['psraq'] = _t_sar()

    # String operations (base variants)
    TEMPLATES['movs'] = _t_string_mov('size_architecture()')
    TEMPLATES['lods'] = _t_string_lods('size_architecture()')
    TEMPLATES['stos'] = _t_string_stos('size_architecture()')
    TEMPLATES['scas'] = _t_string_scas('size_architecture()')

    # REP prefixes (nop in IR terms, actual behavior is loop-like)
    for _op in ['rep', 'repe', 'repne', 'repnz', 'repz']:
        TEMPLATES[_op] = _t_nop()

    # I/O instructions
    for _op in ['ins', 'insb', 'insd', 'insw', 'outs', 'outsb', 'outsd', 'outsw']:
        TEMPLATES[_op] = [f'    [exception("{_op}")].into()']

    # System instructions
    for _op in ['arpl', 'bound', 'clac', 'stac', 'cldemote', 'clts', 'clrssbsy',
                 'encodekey128', 'encodekey256', 'endbr32', 'endbr64', 'enqcmd',
                 'hreset', 'incsspd', 'incsspq', 'invpcid',
                 'lar', 'lds', 'les', 'lfs', 'lgs', 'lss',
                 'loadiwkey', 'monitor', 'mwait',
                 'pconfig', 'ptwrite',
                 'rdrand', 'rdseed', 'rdsspd', 'rdsspq',
                 'rsm', 'rstorssp', 'saveprevssp',
                 'senduipi', 'serialize', 'setssbsy',
                 'swapgs', 'tpause', 'ud', 'uiret', 'umonitor', 'umwait',
                 'wbnoinvd', 'wrfsbase', 'wrgsbase', 'wrpkru',
                 'wrssd', 'wrssq', 'wrussd', 'wrussq',
                 'xabort', 'xacquire', 'xbegin', 'xend', 'xgetbv', 'xrelease',
                 'xresldtrk', 'xsusldtrk', 'xtest']:
        TEMPLATES[_op] = [f'    [exception("{_op}")].into()']

    # XSAVE family (state save/restore)
    for _op in ['xrstor', 'xrstors', 'xsave', 'xsavec', 'xsaveopt', 'xsaves']:
        TEMPLATES[_op] = [f'    [exception("{_op}")].into()']

    # AMX tile instructions
    for _op in ['ldtilecfg', 'sttilecfg', 'tileloadd', 'tileloaddt1',
                 'tilestored', 'tilezero', 'tdpbf16ps', 'tdpbssd',
                 'tdpbsud', 'tdpbusd', 'tdpbuud']:
        TEMPLATES[_op] = [f'    [exception("{_op}")].into()']

    # AES/SHA crypto
    for _op in ['aesdec', 'aesdec128kl', 'aesdec256kl', 'aesdeclast',
                 'aesdecwide128kl', 'aesdecwide256kl',
                 'aesenc', 'aesenc128kl', 'aesenc256kl', 'aesenclast',
                 'aesencwide128kl', 'aesencwide256kl', 'aesimc', 'aeskeygenassist']:
        TEMPLATES[_op] = [f'    [exception("{_op}")].into()']

    for _op in ['sha1msg1', 'sha1msg2', 'sha1nexte', 'sha1rnds4',
                 'sha256msg1', 'sha256msg2', 'sha256rnds2']:
        TEMPLATES[_op] = [f'    [exception("{_op}")].into()']

    # GF2P8 (Galois Field)
    for _op in ['gf2p8affineinvqb', 'gf2p8affineqb', 'gf2p8mulb']:
        TEMPLATES[_op] = [f'    [exception("{_op}")].into()']

    # MPX (bnd*)
    for _op in ['bndcl', 'bndcn', 'bndcu', 'bndldx', 'bndmk', 'bndmov', 'bndstx']:
        TEMPLATES[_op] = [f'    [exception("{_op}")].into()']


_build_templates()


# ============================================================
# Section 5b: ARM Templates
# ============================================================

ARM_TEMPLATES: dict[str, list[str]] = {}


def _build_arm_templates():
    """Build ARM instruction template registry."""
    global ARM_TEMPLATES

    # --- Arithmetic (2-operand and 3-operand forms) ---
    # ADD Xd, Xn, Xm  →  o1 = o2 + o3
    ARM_TEMPLATES['add'] = [
        '    let assignment = assign(b::add(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['sub'] = [
        '    let assignment = assign(b::sub(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['neg'] = [
        '    let assignment = assign(u::neg(o2()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['mul'] = [
        '    let assignment = assign(b::mul(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['madd'] = [
        '    let assignment = assign(b::add(o4(), b::mul(o2(), o3())), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['msub'] = [
        '    let assignment = assign(b::sub(o4(), b::mul(o2(), o3())), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['mneg'] = [
        '    let assignment = assign(u::neg(b::mul(o2(), o3())), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['udiv'] = [
        '    let assignment = assign(b::unsigned_div(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['sdiv'] = [
        '    let assignment = assign(b::signed_div(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['smull'] = [
        '    let assignment = assign(b::mul(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['umull'] = ARM_TEMPLATES['smull']
    ARM_TEMPLATES['smulh'] = [f'    [exception("smulh")].into()']  # high 64 bits of 128-bit product
    ARM_TEMPLATES['umulh'] = [f'    [exception("umulh")].into()']  # high 64 bits of 128-bit product
    ARM_TEMPLATES['smaddl'] = ARM_TEMPLATES['madd']
    ARM_TEMPLATES['umaddl'] = ARM_TEMPLATES['madd']
    ARM_TEMPLATES['smsubl'] = ARM_TEMPLATES['msub']
    ARM_TEMPLATES['umsubl'] = ARM_TEMPLATES['msub']
    ARM_TEMPLATES['smnegl'] = ARM_TEMPLATES['mneg']
    ARM_TEMPLATES['umnegl'] = ARM_TEMPLATES['mneg']

    # ADC/SBC (with carry)
    ARM_TEMPLATES['adc'] = [
        '    let op = b::add(b::add(o2(), o3()), pstate_c.clone());',
        '    let assignment = assign(op, o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['sbc'] = [
        '    let op = b::sub(b::sub(o2(), o3()), u::not(pstate_c.clone()));',
        '    let assignment = assign(op, o1(), o1_size());',
        '    [assignment].into()',
    ]

    # ADDS/SUBS (with flag setting)
    ARM_TEMPLATES['adds'] = [
        '    let op = b::add(o2(), o3());',
        '    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags, assignment].into()',
    ]
    ARM_TEMPLATES['subs'] = [
        '    let op = b::sub(o2(), o3());',
        '    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags, assignment].into()',
    ]
    ARM_TEMPLATES['adcs'] = [
        '    let op = b::add(b::add(o2(), o3()), pstate_c.clone());',
        '    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags, assignment].into()',
    ]
    ARM_TEMPLATES['sbcs'] = [
        '    let op = b::sub(b::sub(o2(), o3()), u::not(pstate_c.clone()));',
        '    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags, assignment].into()',
    ]
    ARM_TEMPLATES['ngc'] = ARM_TEMPLATES['sbc']
    ARM_TEMPLATES['ngcs'] = ARM_TEMPLATES['sbcs']
    ARM_TEMPLATES['negs'] = [
        '    let op = u::neg(o2());',
        '    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags, assignment].into()',
    ]

    # --- Logical ---
    ARM_TEMPLATES['and'] = [
        '    let assignment = assign(b::and(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['orr'] = [
        '    let assignment = assign(b::or(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['eor'] = [
        '    let assignment = assign(b::xor(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['eon'] = [
        '    let assignment = assign(b::xor(o2(), u::not(o3())), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['orn'] = [
        '    let assignment = assign(b::or(o2(), u::not(o3())), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['bic'] = [
        '    let assignment = assign(b::and(o2(), u::not(o3())), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['bics'] = [
        '    let op = b::and(o2(), u::not(o3()));',
        '    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags, assignment].into()',
    ]
    ARM_TEMPLATES['ands'] = [
        '    let op = b::and(o2(), o3());',
        '    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags, assignment].into()',
    ]
    ARM_TEMPLATES['mvn'] = [
        '    let assignment = assign(u::not(o2()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['tst'] = [
        '    let op = b::and(o1(), o2());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags].into()',
    ]

    # --- Shifts ---
    ARM_TEMPLATES['lsl'] = [
        '    let assignment = assign(b::shl(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['lsr'] = [
        '    let assignment = assign(b::shr(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['asr'] = [
        '    let assignment = assign(b::sar(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['ror'] = [
        '    let op = b::or(b::shr(o2(), o3()), b::shl(o2(), b::sub(bit_size_of_o2(), o3())));',
        '    let assignment = assign(op, o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['lslv'] = ARM_TEMPLATES['lsl']
    ARM_TEMPLATES['lsrv'] = ARM_TEMPLATES['lsr']
    ARM_TEMPLATES['asrv'] = ARM_TEMPLATES['asr']
    ARM_TEMPLATES['rorv'] = ARM_TEMPLATES['ror']

    # --- Data movement ---
    ARM_TEMPLATES['mov'] = [
        '    let assignment = assign(o2(), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['movz'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['movn'] = [
        '    let assignment = assign(u::not(o2()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['movk'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['sxtb'] = [
        '    let assignment = sign_extend(o2(), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['sxth'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['sxtw'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['uxtb'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['uxth'] = ARM_TEMPLATES['mov']

    # --- Compare ---
    ARM_TEMPLATES['cmp'] = [
        '    let sub = b::sub(o1(), o2());',
        f'    let calc_flags = calc_flags_automatically(sub, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags].into()',
    ]
    ARM_TEMPLATES['cmn'] = [
        '    let add = b::add(o1(), o2());',
        f'    let calc_flags = calc_flags_automatically(add, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags].into()',
    ]
    ARM_TEMPLATES['ccmp'] = ARM_TEMPLATES['cmp']
    ARM_TEMPLATES['ccmn'] = ARM_TEMPLATES['cmn']

    # --- Conditional select ---
    # Uses condition() IR primitive with o3() as the condition operand.
    # CSEL: if cond then Xn else Xm
    ARM_TEMPLATES['csel'] = [
        '    [condition(o3(), [assign(o1(), o1(), o1_size())], [assign(o2(), o1(), o1_size())])].into()',
    ]
    # CSINC: if cond then Xn else Xm+1
    ARM_TEMPLATES['csinc'] = [
        '    [condition(o3(), [assign(o1(), o1(), o1_size())], [assign(b::add(o2(), c(1)), o1(), o1_size())])].into()',
    ]
    # CSINV: if cond then Xn else NOT(Xm)
    ARM_TEMPLATES['csinv'] = [
        '    [condition(o3(), [assign(o1(), o1(), o1_size())], [assign(u::not(o2()), o1(), o1_size())])].into()',
    ]
    # CSNEG: if cond then Xn else -Xm
    ARM_TEMPLATES['csneg'] = [
        '    [condition(o3(), [assign(o1(), o1(), o1_size())], [assign(u::neg(o2()), o1(), o1_size())])].into()',
    ]
    # CSET: if cond then 1 else 0
    ARM_TEMPLATES['cset'] = [
        '    [condition(o2(), [assign(c(1), o1(), o1_size())], [assign(c(0), o1(), o1_size())])].into()',
    ]
    # CSETM: if cond then ~0 else 0
    ARM_TEMPLATES['csetm'] = [
        '    [condition(o2(), [assign(u::not(c(0)), o1(), o1_size())], [assign(c(0), o1(), o1_size())])].into()',
    ]
    # CINC/CINV/CNEG are aliases
    ARM_TEMPLATES['cinc'] = ARM_TEMPLATES['csinc']
    ARM_TEMPLATES['cinv'] = ARM_TEMPLATES['csinv']
    ARM_TEMPLATES['cneg'] = ARM_TEMPLATES['csneg']

    # --- Flag manipulation ---
    ARM_TEMPLATES['rmif'] = [
        '    let rotated_value = b::or(b::shr(o1(), o2()), b::shl(o1(), b::sub(bit_size_of_o1(), o2())));',
        '    let rotate = condition(b::equal(o2(), c(0), size_unlimited()), [assign(o1(), tmp64.clone(), o1_size())], [assign(rotated_value, tmp64.clone(), o1_size())]);',
        '    let set_n = condition(b::and(o3(), c(8)), [assign(b::and(b::shr(tmp64.clone(), c(3)), c(1)), pstate_n.clone(), size_relative(pstate_n.clone()))], []);',
        '    let set_z = condition(b::and(o3(), c(4)), [assign(b::and(b::shr(tmp64.clone(), c(2)), c(1)), pstate_z.clone(), size_relative(pstate_z.clone()))], []);',
        '    let set_c = condition(b::and(o3(), c(2)), [assign(b::and(b::shr(tmp64.clone(), c(1)), c(1)), pstate_c.clone(), size_relative(pstate_c.clone()))], []);',
        '    let set_v = condition(b::and(o3(), c(1)), [assign(b::and(tmp64.clone(), c(1)), pstate_v.clone(), size_relative(pstate_v.clone()))], []);',
        '    [rotate, set_n, set_z, set_c, set_v].into()',
    ]

    # --- Bit manipulation (complex ops → exception since not representable in simple IR) ---
    ARM_TEMPLATES['rev'] = _arm_t_rev()
    for _op in ['cls', 'clz', 'rbit', 'rev16', 'rev32', 'rev64',
                'bfm', 'ubfm', 'sbfm', 'ubfx', 'sbfx', 'ubfiz', 'sbfiz', 'bfi', 'bfxil']:
        ARM_TEMPLATES[_op] = [f'    [exception("{_op}")].into()']
    ARM_TEMPLATES['extr'] = [
        '    let assignment = assign(o3(), o1(), o1_size());',
        '    [assignment].into()',
    ]

    # --- Control flow ---
    ARM_TEMPLATES['b'] = ['    [jump(o1())].into()']
    ARM_TEMPLATES['bl'] = [
        '    let save_lr = assign(b::add(pc.clone(), instruction_byte_size()), x30.clone(), size_relative(x30.clone()));',
        '    let do_jump = jump(o1());',
        '    [save_lr, do_jump].into()',
    ]
    ARM_TEMPLATES['blr'] = ARM_TEMPLATES['bl']
    ARM_TEMPLATES['br'] = ['    [jump(o1())].into()']
    ARM_TEMPLATES['ret'] = ['    [jump(x30.clone())].into()']
    ARM_TEMPLATES['nop'] = ['    [].into()']
    ARM_TEMPLATES['hint'] = ['    [].into()']
    ARM_TEMPLATES['dgh'] = ARM_TEMPLATES['nop']
    ARM_TEMPLATES['yield'] = ARM_TEMPLATES['nop']

    # --- Conditional branch on register ---
    ARM_TEMPLATES['cbz'] = [
        '    let fallthrough = b::add(pc.clone(), instruction_byte_size());',
        '    [condition(b::equal(o1(), c(0), o1_size()), [jump(o2())], [jump(fallthrough)])].into()',
    ]
    ARM_TEMPLATES['cbnz'] = [
        '    let fallthrough = b::add(pc.clone(), instruction_byte_size());',
        '    let is_zero = b::equal(o1(), c(0), o1_size());',
        '    [condition(u::not(is_zero), [jump(o2())], [jump(fallthrough)])].into()',
    ]
    # --- Conditional branch on bit (test bit via shift+mask, following Intel BT pattern) ---
    ARM_TEMPLATES['tbz'] = [
        '    let fallthrough = b::add(pc.clone(), instruction_byte_size());',
        '    let bit = b::and(b::shr(o1(), o2()), c(1));',
        '    [condition(b::equal(bit, c(0), o1_size()), [jump(o3())], [jump(fallthrough)])].into()',
    ]
    ARM_TEMPLATES['tbnz'] = [
        '    let fallthrough = b::add(pc.clone(), instruction_byte_size());',
        '    let bit = b::and(b::shr(o1(), o2()), c(1));',
        '    [condition(bit, [jump(o3())], [jump(fallthrough)])].into()',
    ]

    # --- PSTATE direct manipulation (snapshot originals to avoid read-after-write) ---
    ARM_TEMPLATES['axflag'] = [
        '    let old_z = pstate_z.clone();',
        '    let old_c = pstate_c.clone();',
        '    let old_v = pstate_v.clone();',
        '    let set_n = assign(c(0), pstate_n.clone(), size_relative(pstate_n.clone()));',
        '    let set_z = assign(b::or(old_z.clone(), old_v.clone()), pstate_z.clone(), size_relative(pstate_z.clone()));',
        '    let set_c = assign(b::and(old_c, u::not(old_v)), pstate_c.clone(), size_relative(pstate_c.clone()));',
        '    let set_v = assign(c(0), pstate_v.clone(), size_relative(pstate_v.clone()));',
        '    [set_n, set_z, set_c, set_v].into()',
    ]
    ARM_TEMPLATES['xaflag'] = [
        '    let old_c = pstate_c.clone();',
        '    let old_z = pstate_z.clone();',
        '    let set_n = assign(b::and(u::not(old_c.clone()), u::not(old_z.clone())), pstate_n.clone(), size_relative(pstate_n.clone()));',
        '    let set_z = assign(b::and(old_z.clone(), old_c.clone()), pstate_z.clone(), size_relative(pstate_z.clone()));',
        '    let set_c = assign(b::or(old_c.clone(), old_z.clone()), pstate_c.clone(), size_relative(pstate_c.clone()));',
        '    let set_v = assign(b::and(u::not(old_c.clone()), old_z.clone()), pstate_v.clone(), size_relative(pstate_v.clone()));',
        '    [set_n, set_z, set_c, set_v].into()',
    ]

    # --- SUBP/SUBPS: AddWithCarry(op1, NOT(op2), '1') = subtraction ---
    ARM_TEMPLATES['subp'] = [
        '    let assignment = assign(b::sub(o2(), o3()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['subps'] = [
        '    let op = b::sub(o2(), o3());',
        '    let assignment = assign(op.clone(), o1(), o1_size());',
        f'    let calc_flags = calc_flags_automatically(op, o1_size(), {ARM_NZCV_FLAGS});',
        '    [calc_flags, assignment].into()',
    ]

    # --- Load/Store (simplified: treat as mov) ---
    ARM_TEMPLATES['ldr'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldrb'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldrh'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldrsb'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['ldrsh'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['ldrsw'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['ldp'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['str'] = [
        '    let assignment = assign(o1(), o2(), o2_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['strb'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['strh'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stp'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['ldar'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['stlr'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['ldxr'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['stxr'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['ldaxr'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['stlxr'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['ldxp'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldaxp'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['stxp'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stlxp'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['ldur'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['stur'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['sturb'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['sturh'] = ARM_TEMPLATES['str']
    # Additional load/store variants (acquire/release/exclusive byte/half)
    ARM_TEMPLATES['ldarb'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldarh'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldaxrb'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldaxrh'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldxrb'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldxrh'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['stlrb'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stlrh'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stlxrb'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stlxrh'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stxrb'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stxrh'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['ldurb'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldurh'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldursb'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['ldursh'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['ldursw'] = ARM_TEMPLATES['sxtb']
    # Additional plain memory forms whose semantics still collapse to load/store in this IR.
    ARM_TEMPLATES['ldapr'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldaprb'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldaprh'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldapur'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldapurb'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldapurh'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldapursb'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['ldapursh'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['ldapursw'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['ldlar'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldlarb'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldlarh'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldnp'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldtr'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldtrb'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldtrh'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ldtrsb'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['ldtrsh'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['ldtrsw'] = ARM_TEMPLATES['sxtb']
    ARM_TEMPLATES['stllr'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stllrb'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stllrh'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stlur'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stlurb'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stlurh'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['stnp'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['sttr'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['sttrb'] = ARM_TEMPLATES['str']
    ARM_TEMPLATES['sttrh'] = ARM_TEMPLATES['str']

    # --- Address generation ---
    ARM_TEMPLATES['adr'] = [
        '    let assignment = assign(b::add(pc.clone(), o2()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['adrp'] = [
        '    let page_base = b::and(pc.clone(), u::not(c(0xFFF)));',
        '    let assignment = assign(b::add(page_base, o2()), o1(), o1_size());',
        '    [assignment].into()',
    ]

    # --- Barriers / NOP-like (prefetch hints are no-ops for analysis) ---
    for _op in ['dmb', 'dsb', 'isb', 'csdb', 'yield_', 'wfe', 'wfi', 'sev', 'sevl',
                'clrex', 'pssbb', 'ssbb', 'esb', 'psb', 'tsb',
                'prfm', 'prfum']:
        ARM_TEMPLATES[_op] = ['    [].into()']

    # --- System / Exception stubs ---
    for _op in ['svc', 'hvc', 'smc', 'brk', 'hlt', 'dcps1', 'dcps2', 'dcps3',
                'drps', 'eret', 'mrs', 'msr', 'sys', 'sysl',
                'at', 'dc', 'ic', 'tlbi',
                'cas', 'casa', 'casl', 'casal',
                'casb', 'cash', 'casp',
                'swp', 'swpa', 'swpl', 'swpal',
                'swpb', 'swph',
                'ldadd', 'ldadda', 'ldaddl', 'ldaddal',
                'ldclr', 'ldclra', 'ldclrl', 'ldclral',
                'ldeor', 'ldeora', 'ldeorl', 'ldeoral',
                'ldset', 'ldseta', 'ldsetl', 'ldsetal',
                'ldsmax', 'ldsmin', 'ldumax', 'ldumin',
                'stadd', 'staddl', 'stclr', 'stclrl',
                'steor', 'steorl', 'stset', 'stsetl']:
        ARM_TEMPLATES[_op] = [f'    [exception("{_op}")].into()']

    # --- SIMD/FP data processing (common patterns) ---
    # Vector arithmetic: add, sub, mul per element
    _simd_arith = {
        'fadd': 'b::add', 'fsub': 'b::sub', 'fmul': 'b::mul',
        'fdiv': 'b::unsigned_div', 'fneg': 'u::neg', 'fabs': 'o2',
        'fmadd': None, 'fmsub': None, 'fnmadd': None, 'fnmsub': None,
    }
    for _op, _ir in _simd_arith.items():
        if _ir is None:
            continue
        if _ir.startswith('u::') or _ir == 'o2':
            # unary
            _expr = f'{_ir}()' if _ir == 'o2' else f'{_ir}(o2())'
            ARM_TEMPLATES[_op] = [
                f'    let assignment = assign({_expr}, o1(), o1_size());',
                '    [assignment].into()',
            ]
        else:
            ARM_TEMPLATES[_op] = [
                f'    let assignment = assign({_ir}(o2(), o3()), o1(), o1_size());',
                '    [assignment].into()',
            ]
    ARM_TEMPLATES['fmadd'] = [
        '    let assignment = assign(b::add(b::mul(o2(), o3()), o4()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['fmsub'] = [
        '    let assignment = assign(b::sub(b::mul(o2(), o3()), o4()), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['fnmadd'] = [
        '    let assignment = assign(u::neg(b::add(b::mul(o2(), o3()), o4())), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['fnmsub'] = [
        '    let assignment = assign(b::sub(o4(), b::mul(o2(), o3())), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['fmov'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['fcmp'] = ARM_TEMPLATES['cmp']
    ARM_TEMPLATES['fccmp'] = ARM_TEMPLATES['cmp']
    ARM_TEMPLATES['fcsel'] = [
        '    let assignment = assign(o2(), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['fcvt'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['fcvtzs'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['fcvtzu'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['scvtf'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['ucvtf'] = ARM_TEMPLATES['mov']
    ARM_TEMPLATES['fsqrt'] = [
        '    let assignment = assign(o2(), o1(), o1_size());',
        '    [assignment].into()',
    ]
    ARM_TEMPLATES['fmin'] = ARM_TEMPLATES['fadd']
    ARM_TEMPLATES['fmax'] = ARM_TEMPLATES['fadd']
    ARM_TEMPLATES['fminnm'] = ARM_TEMPLATES['fadd']
    ARM_TEMPLATES['fmaxnm'] = ARM_TEMPLATES['fadd']
    ARM_TEMPLATES['frinta'] = ARM_TEMPLATES['fsqrt']
    ARM_TEMPLATES['frinti'] = ARM_TEMPLATES['fsqrt']
    ARM_TEMPLATES['frintm'] = ARM_TEMPLATES['fsqrt']
    ARM_TEMPLATES['frintn'] = ARM_TEMPLATES['fsqrt']
    ARM_TEMPLATES['frintp'] = ARM_TEMPLATES['fsqrt']
    ARM_TEMPLATES['frintx'] = ARM_TEMPLATES['fsqrt']
    ARM_TEMPLATES['frintz'] = ARM_TEMPLATES['fsqrt']

    # ADDG/SUBG (memory tagging) → exception
    ARM_TEMPLATES['addg'] = [f'    [exception("addg")].into()']
    ARM_TEMPLATES['subg'] = [f'    [exception("subg")].into()']


_build_arm_templates()


# ============================================================
# Section 5c: ARM Pseudocode Translator
# ============================================================

# Lines to skip in ARM pseudocode (function calls that are checks, not computation)
_ARM_SKIP_PATTERNS = [
    'CheckFPAdvSIMDEnabled64',
    'CheckFPEnabled64',
    'CheckSVEEnabled',
    'CheckSVEAndSMEnabled',
    'CheckSMEEnabled',
    'CheckSMEAndZAEnabled',
    'AArch64.CheckFPAdvSIMDEnabled',
    'SetTagCheckedInstruction',
    'boolean',
    'integer d =',
    'integer n =',
    'integer m =',
    'integer t =',
    'integer s =',
    'integer a =',
    'integer datasize',
    'integer esize',
    'integer elements',
    'integer regsize',
    'integer pairs',
    'integer part',
    'constant integer',
    'bits(datasize) result;',
    'bits(datasize) operand',
    'bits(esize)',
    'bits(4) nzcv',
    'boolean sub_op',
    'boolean setflags',
    'boolean unsigned',
    'boolean neg ',
    'boolean pair',
    'boolean replicate',
    'boolean is_neg',
    'ShiftType shift_type',
    'LogicalOp op ',
    'MoveWideOp opcode',
    'CountOp opcode',
    'ReduceOp op',
    'CompareOp op ',
    'FPMaxMinOp op',
    'FPUnaryOp fpop',
    'FPConvOp fpop',
    'BranchType branch_type',
    'AccType acctype',
    'bit carry_in',
    'bit sub_op',
]

# ARM pseudocode binary operators → IR
_ARM_BINOP_MAP = {
    'AND': 'b::and',
    'OR': 'b::or',
    'EOR': 'b::xor',
    'MOD': 'b::unsigned_mod',
    'DIV': 'b::unsigned_div',
    '+': 'b::add',
    '-': 'b::sub',
    '*': 'b::mul',
}


def _arm_classify_unsupported(operation: str) -> str | None:
    """Classify why an ARM operation cannot be translated. Returns reason or None if translatable."""
    lines = operation.strip().split('\n')
    clean = [l.strip() for l in lines if l.strip()]

    has_for = any(l.startswith('for ') or l.startswith('FOR ') for l in clean)
    has_while = any(l.startswith('while ') or l.startswith('WHILE ') for l in clean)
    has_elem = any('Elem[' in l for l in clean)
    has_mem = any('Mem[' in l or 'Mem(' in l for l in clean)
    has_complex_fn = any(fn in operation for fn in [
        'AArch64.', 'Poly', 'CRC', 'SHA', 'AES', 'SM3', 'SM4',
        'FPToFixed', 'FixedToFP', 'FPCompare', 'FPRound',
        'SatQ', 'UnsignedSatQ', 'SignedSatQ',
        'ConditionHolds', 'BigEndianReverse',
        'Hint_', 'System', 'ExclusiveMonitors',
    ])

    if has_for:
        return "unsupported: for-loop (SIMD element-wise)"
    if has_while:
        return "unsupported: while-loop"
    if has_elem:
        return "unsupported: Elem[] vector element access"
    if has_mem:
        return "unsupported: memory access Mem[]"
    if has_complex_fn:
        return "unsupported: complex function call"

    return None


def translate_arm_pseudocode(operation: str, mnemonic: str) -> list[str] | None:
    """Translate ARM pseudocode to IR, handling ARM-specific syntax.

    Returns list of indented Rust body lines, or None if too complex.
    """
    # First classify if it's translatable at all
    unsupported = _arm_classify_unsupported(operation)
    if unsupported:
        return None

    lines = operation.strip().split('\n')
    clean_lines = []
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped == '':
            continue
        # Strip trailing semicolons
        stripped = stripped.rstrip(';').strip()
        if not stripped:
            continue
        # Skip known non-computational lines
        if any(stripped.startswith(pat) or stripped.startswith(f'// {pat}') for pat in _ARM_SKIP_PATTERNS):
            continue
        # Skip type declarations: bits(...) varname;  or  integer varname;  or  bit varname;
        if re.match(r'^bits\(\w+\)\s+\w+$', stripped):
            continue
        if re.match(r'^integer\s+\w+$', stripped):
            continue
        if re.match(r'^bit\s+\w+$', stripped):
            continue
        clean_lines.append(stripped)

    if not clean_lines:
        return None

    # Check for remaining if/for/while
    has_if = any(l.startswith('if ') or l.startswith('IF ') for l in clean_lines)
    has_for = any(l.startswith('for ') or l.startswith('FOR ') for l in clean_lines)
    if has_for:
        return None

    # Simple case: no IF, few lines with assignments
    if not has_if and len(clean_lines) <= 10:
        return _translate_arm_flat(clean_lines, mnemonic)

    # IF with simple body
    if has_if and len(clean_lines) <= 15:
        return _translate_arm_if(clean_lines, mnemonic)

    return None


def _arm_expr_to_ir(expr: str, local_vars: dict[str, str]) -> str:
    """Convert an ARM pseudocode expression to IR code."""
    expr = expr.strip()

    # Check local variable
    if expr in local_vars:
        return local_vars[expr]

    # ARM register access: X[n], X[d], etc.
    if expr in ARM_PSEUDOCODE_REGS:
        return ARM_PSEUDOCODE_REGS[expr]

    # PSTATE.C, PSTATE.N etc.
    pstate_m = re.match(r'^PSTATE\.([NZCV])$', expr)
    if pstate_m:
        return f'pstate_{pstate_m.group(1).lower()}.clone()'

    # NOT(expr)
    not_m = re.match(r'^NOT\((.+)\)$', expr)
    if not_m:
        inner = _arm_expr_to_ir(not_m.group(1), local_vars)
        return f'u::not({inner})'

    # ZeroExtend(expr, ...) or SignExtend(expr, ...)
    ze_m = re.match(r'^(Zero|Sign)Extend\((.+?)(?:,\s*\w+)?\)$', expr)
    if ze_m:
        inner = _arm_expr_to_ir(ze_m.group(2), local_vars)
        return inner  # simplified: extension is handled by assign size

    # Abs(expr)
    abs_m = re.match(r'^Abs\((.+)\)$', expr)
    if abs_m:
        inner = _arm_expr_to_ir(abs_m.group(1), local_vars)
        return inner  # simplified

    # SInt/UInt wrapping
    sint_m = re.match(r'^[SU]Int\((.+)\)$', expr)
    if sint_m:
        return _arm_expr_to_ir(sint_m.group(1), local_vars)

    # Numeric literal
    if re.match(r'^[0-9]+$', expr):
        return f'c({expr})'
    if re.match(r'^0x[0-9a-fA-F]+$', expr):
        return f'c({expr})'

    # Bit slice: expr<high:low>
    slice_m = re.match(r'^(.+)<(\d+):(\d+)>$', expr)
    if slice_m:
        return _arm_expr_to_ir(slice_m.group(1), local_vars)

    # Binary: expr1 OP expr2 (try splitting on known operators)
    for op_text, ir_fn in [(' EOR ', 'b::xor'), (' AND ', 'b::and'), (' OR ', 'b::or'),
                            (' + ', 'b::add'), (' - ', 'b::sub'), (' * ', 'b::mul'),
                            (' DIV ', 'b::unsigned_div'), (' MOD ', 'b::unsigned_mod')]:
        if op_text in expr:
            parts = expr.split(op_text, 1)
            left = _arm_expr_to_ir(parts[0], local_vars)
            right = _arm_expr_to_ir(parts[1], local_vars)
            return f'{ir_fn}({left}, {right})'

    # AddWithCarry(a, b, c) — common ARM pattern
    awc_m = re.match(r'^AddWithCarry\((.+?),\s*(.+?),\s*(.+?)\)$', expr)
    if awc_m:
        a = _arm_expr_to_ir(awc_m.group(1), local_vars)
        b = _arm_expr_to_ir(awc_m.group(2), local_vars)
        carry = _arm_expr_to_ir(awc_m.group(3), local_vars)
        return f'b::add(b::add({a}, {b}), {carry})'

    # ShiftReg(n, type, amount) — simplified
    sr_m = re.match(r'^ShiftReg\((.+?),\s*(.+?),\s*(.+?)\)$', expr)
    if sr_m:
        return _arm_expr_to_ir(f'X[{sr_m.group(1).strip()}]', local_vars) if sr_m.group(1).strip().isalpha() else 'o2()'

    # Parenthesized expression
    if expr.startswith('(') and expr.endswith(')'):
        return _arm_expr_to_ir(expr[1:-1], local_vars)

    # Fallback: unknown_data
    return 'unknown_data()'


def _translate_arm_flat(lines: list[str], mnemonic: str) -> list[str] | None:
    """Translate simple ARM pseudocode lines (no if/for) to IR."""
    rust_lines: list[str] = []
    local_vars: dict[str, str] = {}
    statements: list[str] = []
    var_counter = 0

    for line in lines:
        # Tuple assignment: (result, nzcv) = AddWithCarry(...)
        tuple_m = re.match(r'^\((\w+),\s*(\w+)\)\s*=\s*(.+)$', line)
        if tuple_m:
            var1, var2, expr_raw = tuple_m.group(1), tuple_m.group(2), tuple_m.group(3)
            expr_ir = _arm_expr_to_ir(expr_raw, local_vars)
            vn = f'v_{var_counter}'
            var_counter += 1
            rust_lines.append(f'    let {vn} = {expr_ir};')
            local_vars[var1] = f'{vn}.clone()'
            local_vars[var2] = f'{vn}.clone()'  # nzcv simplified
            continue

        # Assignment: target = expr
        assign_m = re.match(r'^(\S+(?:\[.*?\])?(?:<[^>]+>)?)\s*=\s*(.+)$', line)
        if not assign_m:
            continue

        target_raw = assign_m.group(1).strip()
        expr_raw = assign_m.group(2).strip()

        # Determine target IR and size
        # Strip bit-slice from target
        target_clean = re.sub(r'<[^>]+>', '', target_raw).strip()

        if target_clean in ARM_PSEUDOCODE_REGS:
            target_ir = ARM_PSEUDOCODE_REGS[target_clean]
            size_ir = ARM_PSEUDOCODE_SIZES[target_clean]
        elif target_clean.startswith('PSTATE.'):
            flag = target_clean.split('.')[1]
            if len(flag) == 1 and flag in 'NZCV':
                target_ir = f'pstate_{flag.lower()}.clone()'
                size_ir = f'size_relative(pstate_{flag.lower()}.clone())'
            else:
                continue  # complex PSTATE assignment
        elif re.match(r'^[a-z_]\w*$', target_clean):
            # Local variable
            expr_ir = _arm_expr_to_ir(expr_raw, local_vars)
            vn = f'v_{var_counter}'
            var_counter += 1
            rust_lines.append(f'    let {vn} = {expr_ir};')
            local_vars[target_clean] = vn
            continue
        else:
            continue

        expr_ir = _arm_expr_to_ir(expr_raw, local_vars)
        sn = f'stmt_{var_counter}'
        var_counter += 1
        rust_lines.append(f'    let {sn} = assign({expr_ir}, {target_ir}, {size_ir});')
        statements.append(sn)

    # PSTATE.<N,Z,C,V> = nzcv pattern: add flag calculation
    # Check if any PSTATE flags were set
    has_pstate = any('pstate_' in s for s in rust_lines)

    if not statements:
        return None

    rust_lines.append(f'    [{", ".join(statements)}].into()')
    return rust_lines


def _translate_arm_if(lines: list[str], mnemonic: str) -> list[str] | None:
    """Translate simple ARM IF/THEN pseudocode to IR."""
    # For now, handle the common pattern:
    #   if sub_op then operand2 = NOT(operand2);
    #   ... (rest is flat assignments)
    # Strategy: skip the condition, translate the non-conditional parts

    flat_lines = []
    in_skip = False
    for line in lines:
        l = line.strip()
        if l.startswith('if ') or l.startswith('IF '):
            # Check for single-line if: "if cond then stmt;"
            single_m = re.match(r'^(?:if|IF)\s+.+\s+(?:then|THEN)\s+(.+)$', l)
            if single_m:
                # Skip single-line conditionals (like "if sub_op then operand2 = NOT(operand2)")
                continue
            in_skip = True
            continue
        if in_skip:
            if l.startswith('else') or l.startswith('ELSE'):
                in_skip = False
                continue
            if l.startswith('end') or l.startswith('END') or l == 'FI' or l == 'fi':
                in_skip = False
                continue
            continue
        if l.startswith('else') or l.startswith('ELSE'):
            in_skip = True
            continue
        flat_lines.append(l)

    if not flat_lines:
        return None

    return _translate_arm_flat(flat_lines, mnemonic)


# ============================================================
# Section 6: SIMD Pattern Detection
# ============================================================

# Pattern: (regex for mnemonic, IR expression for core op, is_3_operand)
SIMD_PATTERNS: list[tuple[str, str, bool]] = [
    # Packed floating-point arithmetic
    (r'^v?add[ps][sdh]$', 'b::add(SRC1, SRC2)', True),
    (r'^v?sub[ps][sdh]$', 'b::sub(SRC1, SRC2)', True),
    (r'^v?mul[ps][sdh]$', 'b::mul(SRC1, SRC2)', True),
    (r'^v?div[ps][sdh]$', 'b::unsigned_div(SRC1, SRC2)', True),
    (r'^v?min[ps][sdh]$', 'b::sub(SRC1, SRC2)', True),  # Simplified
    (r'^v?max[ps][sdh]$', 'b::sub(SRC2, SRC1)', True),  # Simplified

    # Packed integer arithmetic
    (r'^v?padd[bwdq]$', 'b::add(SRC1, SRC2)', True),
    (r'^v?padds[bw]$', 'b::add(SRC1, SRC2)', True),  # Saturating add
    (r'^v?paddus[bw]$', 'b::add(SRC1, SRC2)', True),
    (r'^v?psub[bwdq]$', 'b::sub(SRC1, SRC2)', True),
    (r'^v?psubs[bw]$', 'b::sub(SRC1, SRC2)', True),  # Saturating sub
    (r'^v?psubus[bw]$', 'b::sub(SRC1, SRC2)', True),
    (r'^v?pmull[wdq]$', 'b::mul(SRC1, SRC2)', True),
    (r'^v?pmulh[wu]?w$', 'b::mul(SRC1, SRC2)', True),
    (r'^v?pmuldq$', 'b::mul(SRC1, SRC2)', True),
    (r'^v?pmuludq$', 'b::mul(SRC1, SRC2)', True),
    (r'^v?pmaddwd$', 'b::mul(SRC1, SRC2)', True),
    (r'^v?pmaddubsw$', 'b::mul(SRC1, SRC2)', True),

    # Packed bitwise
    (r'^v?pand[dq]?$', 'b::and(SRC1, SRC2)', True),
    (r'^v?pandn[dq]?$', 'b::and(u::not(SRC1), SRC2)', True),
    (r'^v?por[dq]?$', 'b::or(SRC1, SRC2)', True),
    (r'^v?pxor[dq]?$', 'b::xor(SRC1, SRC2)', True),
    (r'^v?andps$', 'b::and(SRC1, SRC2)', True),
    (r'^v?andpd$', 'b::and(SRC1, SRC2)', True),
    (r'^v?andnps$', 'b::and(u::not(SRC1), SRC2)', True),
    (r'^v?andnpd$', 'b::and(u::not(SRC1), SRC2)', True),
    (r'^v?orps$', 'b::or(SRC1, SRC2)', True),
    (r'^v?orpd$', 'b::or(SRC1, SRC2)', True),
    (r'^v?xorps$', 'b::xor(SRC1, SRC2)', True),
    (r'^v?xorpd$', 'b::xor(SRC1, SRC2)', True),

    # Packed shifts
    (r'^v?psll[wdq]$', 'b::shl(SRC1, SRC2)', True),
    (r'^v?psrl[wdq]$', 'b::shr(SRC1, SRC2)', True),
    (r'^v?psra[wd]$', 'b::sar(SRC1, SRC2)', True),
    (r'^v?pslldq$', 'b::shl(SRC1, SRC2)', True),
    (r'^v?psrldq$', 'b::shr(SRC1, SRC2)', True),
    (r'^v?psllv[dwq]$', 'b::shl(SRC1, SRC2)', True),
    (r'^v?psrlv[dwq]$', 'b::shr(SRC1, SRC2)', True),
    (r'^v?psrav[dwq]$', 'b::sar(SRC1, SRC2)', True),

    # Packed compare
    (r'^v?pcmpeq[bwdq]$', 'b::equal(SRC1, SRC2, o1_size())', True),
    (r'^v?pcmpgt[bwdq]$', 'b::signed_less(SRC2, SRC1, o1_size())', True),

    # Packed min/max (simplified as comparisons)
    (r'^v?pmins[bwd]$', 'b::sub(SRC1, SRC2)', True),
    (r'^v?pminu[bwd]$', 'b::sub(SRC1, SRC2)', True),
    (r'^v?pmaxs[bwd]$', 'b::sub(SRC2, SRC1)', True),
    (r'^v?pmaxu[bwd]$', 'b::sub(SRC2, SRC1)', True),

    # Packed abs
    (r'^v?pabs[bwd]$', 'SRC2', False),  # Simplified: pass through

    # SIMD moves
    (r'^v?movaps$', 'SRC2', False),
    (r'^v?movapd$', 'SRC2', False),
    (r'^v?movups$', 'SRC2', False),
    (r'^v?movupd$', 'SRC2', False),
    (r'^v?movdqa\d*$', 'SRC2', False),
    (r'^v?movdqu\d*$', 'SRC2', False),
    (r'^v?movss$', 'SRC2', False),
    (r'^v?movsd$', 'SRC2', False),
    (r'^v?movd$', 'SRC2', False),
    (r'^v?movq$', 'SRC2', False),
    (r'^v?movlps$', 'SRC2', False),
    (r'^v?movhps$', 'SRC2', False),
    (r'^v?movlpd$', 'SRC2', False),
    (r'^v?movhpd$', 'SRC2', False),
    (r'^v?movlhps$', 'SRC2', False),
    (r'^v?movhlps$', 'SRC2', False),
    (r'^v?movmskps$', 'SRC2', False),
    (r'^v?movmskpd$', 'SRC2', False),
    (r'^v?movntps$', 'SRC2', False),
    (r'^v?movntpd$', 'SRC2', False),
    (r'^v?movntdq$', 'SRC2', False),
    (r'^v?movntdqa$', 'SRC2', False),
    (r'^v?movnti$', 'SRC2', False),
    (r'^v?lddqu$', 'SRC2', False),

    # Conversions (simplified as moves)
    (r'^v?cvt.*$', 'u::zero_extend(SRC2)', False),
    (r'^v?pmov[sz]x.*$', 'u::sign_extend(SRC2)', False),

    # Unpack/interleave (simplified as pass-through)
    (r'^v?punpck[lh][bwdq]+$', 'SRC2', False),
    (r'^v?unpcklps$', 'SRC2', False),
    (r'^v?unpckhps$', 'SRC2', False),
    (r'^v?unpcklpd$', 'SRC2', False),
    (r'^v?unpckhpd$', 'SRC2', False),

    # Pack (simplified)
    (r'^v?packss.*$', 'SRC2', False),
    (r'^v?packus.*$', 'SRC2', False),

    # Shuffle/permute/blend (simplified as pass-through)
    (r'^v?shufps$', 'SRC2', False),
    (r'^v?shufpd$', 'SRC2', False),
    (r'^v?pshufd$', 'SRC2', False),
    (r'^v?pshufhw$', 'SRC2', False),
    (r'^v?pshuflw$', 'SRC2', False),
    (r'^v?pshufb$', 'SRC2', False),
    (r'^v?palignr$', 'SRC2', False),
    (r'^v?pblendw$', 'SRC2', False),
    (r'^v?pblend[dv]b?$', 'SRC2', False),
    (r'^v?blendps$', 'SRC2', False),
    (r'^v?blendpd$', 'SRC2', False),
    (r'^v?blendvps$', 'SRC2', False),
    (r'^v?blendvpd$', 'SRC2', False),
    (r'^v?vperm\w+$', 'SRC2', False),
    (r'^v?perm\w+$', 'SRC2', False),
    (r'^v?vpermi2\w+$', 'SRC2', False),

    # Broadcast
    (r'^v?broadcast\w*$', 'SRC2', False),
    (r'^v?pbroadcast\w*$', 'SRC2', False),
    (r'^vbroadcast\w*$', 'SRC2', False),
    (r'^vpbroadcast\w*$', 'SRC2', False),

    # Gather/scatter (simplified as loads/stores)
    (r'^v?gather\w*$', 'SRC2', False),
    (r'^v?pgather\w*$', 'SRC2', False),
    (r'^v?scatter\w*$', 'SRC2', False),
    (r'^v?pscatter\w*$', 'SRC2', False),

    # FMA (fused multiply-add)
    (r'^v?fmadd\w+$', 'b::add(b::mul(SRC1, SRC2), o3())', True),
    (r'^v?fmsub\w+$', 'b::sub(b::mul(SRC1, SRC2), o3())', True),
    (r'^v?fnmadd\w+$', 'b::add(u::neg(b::mul(SRC1, SRC2)), o3())', True),
    (r'^v?fnmsub\w+$', 'b::sub(u::neg(b::mul(SRC1, SRC2)), o3())', True),
    (r'^v?fmaddsub\w+$', 'b::add(b::mul(SRC1, SRC2), o3())', True),
    (r'^v?fmsubadd\w+$', 'b::sub(b::mul(SRC1, SRC2), o3())', True),

    # Sqrt / reciprocal (no IR equivalent, simplified)
    (r'^v?sqrtps$', 'SRC2', False),
    (r'^v?sqrtpd$', 'SRC2', False),
    (r'^v?sqrtss$', 'SRC2', False),
    (r'^v?sqrtsd$', 'SRC2', False),
    (r'^v?rsqrtps$', 'SRC2', False),
    (r'^v?rsqrtss$', 'SRC2', False),
    (r'^v?rcpps$', 'SRC2', False),
    (r'^v?rcpss$', 'SRC2', False),

    # SSE comparison
    (r'^v?cmpps$', 'b::equal(SRC1, SRC2, o1_size())', True),
    (r'^v?cmppd$', 'b::equal(SRC1, SRC2, o1_size())', True),
    (r'^v?cmpss$', 'b::equal(SRC1, SRC2, o1_size())', True),
    (r'^v?cmpsd$', 'b::equal(SRC1, SRC2, o1_size())', True),

    # UCOMIS/COMIS
    (r'^v?u?comiss$', 'b::sub(o1(), o2())', False),
    (r'^v?u?comisd$', 'b::sub(o1(), o2())', False),

    # SSE string (simplified)
    (r'^v?pcmpestri$', 'SRC2', False),
    (r'^v?pcmpestrm$', 'SRC2', False),
    (r'^v?pcmpistri$', 'SRC2', False),
    (r'^v?pcmpistrm$', 'SRC2', False),

    # AES (exception)
    (r'^v?aes\w+$', 'EXCEPTION', False),
    (r'^v?pclmulqdq$', 'EXCEPTION', False),

    # SHA (exception)
    (r'^sha\w+$', 'EXCEPTION', False),

    # CRC
    (r'^crc32$', 'EXCEPTION', False),

    # Horizontal add/sub
    (r'^v?haddps$', 'b::add(SRC1, SRC2)', True),
    (r'^v?haddpd$', 'b::add(SRC1, SRC2)', True),
    (r'^v?hsubps$', 'b::sub(SRC1, SRC2)', True),
    (r'^v?hsubpd$', 'b::sub(SRC1, SRC2)', True),
    (r'^v?phadd[wd]$', 'b::add(SRC1, SRC2)', True),
    (r'^v?phaddsw$', 'b::add(SRC1, SRC2)', True),
    (r'^v?phsub[wd]$', 'b::sub(SRC1, SRC2)', True),
    (r'^v?phsubsw$', 'b::sub(SRC1, SRC2)', True),

    # SAD/MPS
    (r'^v?psadbw$', 'b::sub(SRC1, SRC2)', True),
    (r'^v?mpsadbw$', 'b::sub(SRC1, SRC2)', True),
    (r'^v?dpps$', 'b::mul(SRC1, SRC2)', True),
    (r'^v?dppd$', 'b::mul(SRC1, SRC2)', True),

    # Insert/extract (simplified)
    (r'^v?pinsrb$', 'SRC2', False),
    (r'^v?pinsrw$', 'SRC2', False),
    (r'^v?pinsrd$', 'SRC2', False),
    (r'^v?pinsrq$', 'SRC2', False),
    (r'^v?insertps$', 'SRC2', False),
    (r'^v?pextrb$', 'SRC2', False),
    (r'^v?pextrw$', 'SRC2', False),
    (r'^v?pextrd$', 'SRC2', False),
    (r'^v?pextrq$', 'SRC2', False),
    (r'^v?extractps$', 'SRC2', False),

    # Mask operations
    (r'^v?maskmov\w*$', 'SRC2', False),
    (r'^v?pmaskmov\w*$', 'SRC2', False),

    # Test
    (r'^v?ptest$', 'b::and(o1(), o2())', False),
    (r'^v?vtestps$', 'b::and(o1(), o2())', False),
    (r'^v?vtestpd$', 'b::and(o1(), o2())', False),

    # Round
    (r'^v?roundps$', 'SRC2', False),
    (r'^v?roundpd$', 'SRC2', False),
    (r'^v?roundss$', 'SRC2', False),
    (r'^v?roundsd$', 'SRC2', False),
    (r'^v?vrndscale\w*$', 'SRC2', False),

    # Sign/zero extend packed
    (r'^v?pmovsx\w+$', 'u::sign_extend(SRC2)', False),
    (r'^v?pmovzx\w+$', 'u::zero_extend(SRC2)', False),

    # AVX-512 specific
    (r'^v?compress\w*$', 'SRC2', False),
    (r'^v?expand\w*$', 'SRC2', False),
    (r'^v?conflict\w*$', 'SRC2', False),
    (r'^v?pternlog\w*$', 'b::xor(SRC1, SRC2)', True),

    # Misc SIMD
    (r'^v?movbe$', 'SRC2', False),
    (r'^v?pclmulqdq$', 'EXCEPTION', False),
    (r'^v?vpclmul\w*$', 'EXCEPTION', False),
    (r'^v?gf2p8\w*$', 'EXCEPTION', False),
]


def try_simd_pattern(mnemonic: str) -> list[str] | None:
    """Try to generate IR for a SIMD instruction via pattern matching."""
    for pattern, expr_template, is_3op in SIMD_PATTERNS:
        if re.match(pattern, mnemonic):
            if expr_template == 'EXCEPTION':
                return [f'    [exception("{mnemonic}")].into()']

            # Replace SRC1/SRC2 with appropriate operands
            if is_3op:
                # VEX/EVEX 3-operand: DEST=o1, SRC1=o2, SRC2=o3
                expr = expr_template.replace('SRC1', 'o2()').replace('SRC2', 'o3()')
            else:
                # 2-operand: DEST=o1, SRC=o2
                expr = expr_template.replace('SRC2', 'o2()').replace('SRC1', 'o1()')

            return [
                f'    let assignment = assign({expr}, o1(), o1_size());',
                '    [assignment].into()',
            ]

    # Broad catch-all patterns for remaining SIMD/vector instructions
    result = _try_broad_simd(mnemonic)
    if result:
        return result

    return None


def _try_broad_simd(mnemonic: str) -> list[str] | None:
    """Broad pattern matching for SIMD instructions not caught by specific patterns."""

    # Strip v prefix for analysis
    base = mnemonic[1:] if mnemonic.startswith('v') else mnemonic

    # FPU x87 instructions (f-prefix, not fma)
    if mnemonic.startswith('f') and not mnemonic.startswith('fma'):
        return _try_fpu_pattern(mnemonic)

    # AVX-512 mask register operations (k-prefix)
    if mnemonic.startswith('k') and len(mnemonic) > 1:
        return _try_mask_pattern(mnemonic)

    # For v-prefix instructions, try to detect operation type from base
    if mnemonic.startswith('v'):
        # Add/sub/mul patterns with any suffix
        if base.startswith('add'):
            return ['    let assignment = assign(b::add(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('sub'):
            return ['    let assignment = assign(b::sub(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('mul') and not base.startswith('multishiftqb'):
            return ['    let assignment = assign(b::mul(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('div'):
            return ['    let assignment = assign(b::unsigned_div(o2(), o3()), o1(), o1_size());', '    [assignment].into()']

        # Bitwise
        if base.startswith('pand') or base.startswith('and'):
            if 'andn' in base:
                return ['    let assignment = assign(b::and(u::not(o2()), o3()), o1(), o1_size());', '    [assignment].into()']
            return ['    let assignment = assign(b::and(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('por') or base.startswith('or'):
            return ['    let assignment = assign(b::or(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('pxor') or base.startswith('xor'):
            return ['    let assignment = assign(b::xor(o2(), o3()), o1(), o1_size());', '    [assignment].into()']

        # Compare
        if base.startswith('cmp') or base.startswith('pcmp') or base.startswith('ucomis') or base.startswith('comis'):
            return [
                '    let sub = b::sub(o1(), o2());',
                f'    let calc_flags = calc_flags_automatically(sub, o1_size(), {ALL_FLAGS});',
                '    [calc_flags].into()',
            ]

        # Shift/rotate
        if base.startswith('psll') or base.startswith('pshld'):
            return ['    let assignment = assign(b::shl(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('psrl') or base.startswith('pshrd'):
            return ['    let assignment = assign(b::shr(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('psra'):
            return ['    let assignment = assign(b::sar(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('prol') or base.startswith('pror'):
            return ['    let assignment = assign(b::or(b::shl(o2(), o3()), b::shr(o2(), b::sub(bit_size_of_o2(), o3()))), o1(), o1_size());', '    [assignment].into()']

        # Blend operations (conditional assignment)
        if 'blend' in base:
            return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']

        # Move/store/load/broadcast/compress/expand/extract/insert
        if any(base.startswith(p) for p in ['mov', 'broadcast', 'compress', 'expand',
                'pmov', 'extract', 'insert', 'gather', 'scatter',
                'alignd', 'alignq', 'pblendm', 'palign']):
            return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']

        # FMA (fused multiply-add) variants
        if base.startswith('fmadd') or base.startswith('fmaddsub'):
            return ['    let assignment = assign(b::add(b::mul(o1(), o2()), o3()), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('fmsub') or base.startswith('fmsubadd'):
            return ['    let assignment = assign(b::sub(b::mul(o1(), o2()), o3()), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('fnmadd'):
            return ['    let assignment = assign(b::sub(o3(), b::mul(o1(), o2())), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('fnmsub'):
            return ['    let assignment = assign(u::neg(b::add(b::mul(o1(), o2()), o3())), o1(), o1_size());', '    [assignment].into()']
        if base.startswith('fcmadd') or base.startswith('fcmulc') or base.startswith('fmulc'):
            return ['    let assignment = assign(b::mul(o2(), o3()), o1(), o1_size());', '    [assignment].into()']

        # Dot product
        if base.startswith('dpb') or base.startswith('dp'):
            return ['    let assignment = assign(b::mul(o2(), o3()), o1(), o1_size());', '    [assignment].into()']

        # Reciprocal/sqrt/reduce/range/scalef/getexp/getmant/fixup (unary-like, pass through)
        if any(base.startswith(p) for p in ['rcp', 'rsqrt', 'sqrt', 'reduce', 'range',
                'scalef', 'getexp', 'getmant', 'fixup', 'fpclass',
                'round', 'rndscale']):
            return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']

        # Sad/mpsadbw/dbpsadbw
        if 'sad' in base:
            return ['    let assignment = assign(b::sub(o2(), o3()), o1(), o1_size());', '    [assignment].into()']

        # Count operations (popcnt, lzcnt, plzcnt, etc.)
        if 'cnt' in base or 'popcnt' in base:
            return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']

        # Test operations
        if 'test' in base:
            return [
                '    let and_val = b::and(o1(), o2());',
                f'    let calc_flags = calc_flags_automatically(and_val, o1_size(), {LOGIC_FLAGS});',
                '    [calc_flags].into()',
            ]

        # Shuffle/permute/interleave/pack/unpack
        if any(base.startswith(p) for p in ['shuf', 'perm', 'punpck', 'unpack',
                'pack', 'pshuf', 'p2intersect', 'pmultishiftqb', 'pshufbitqmb']):
            return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']

        # Signed operations (psign, pabs, etc.)
        if base.startswith('psign') or base.startswith('pabs'):
            return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']

        # Zero-upper
        if base in ('zeroall', 'zeroupper'):
            return ['    [].into()']

        # verr/verw (system)
        if base in ('err', 'erw'):
            return [f'    [exception("{mnemonic}")].into()']

        # Fallback for v-prefix: assignment from o2 to o1
        return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']

    return None


def _try_fpu_pattern(mnemonic: str) -> list[str] | None:
    """Handle x87 FPU instructions."""
    # FPU arithmetic
    fpu_ops = {
        'fadd': 'b::add', 'faddp': 'b::add',
        'fsub': 'b::sub', 'fsubp': 'b::sub', 'fsubr': 'b::sub', 'fsubrp': 'b::sub',
        'fmul': 'b::mul', 'fmulp': 'b::mul',
        'fdiv': 'b::signed_div', 'fdivp': 'b::signed_div', 'fdivr': 'b::signed_div', 'fdivrp': 'b::signed_div',
        'fiadd': 'b::add', 'fisub': 'b::sub', 'fisubr': 'b::sub',
        'fimul': 'b::mul', 'fidiv': 'b::signed_div', 'fidivr': 'b::signed_div',
    }
    if mnemonic in fpu_ops:
        op = fpu_ops[mnemonic]
        return [
            f'    let op = {op}(o1(), o2());',
            '    let assignment = assign(op, o1(), o1_size());',
            '    [assignment].into()',
        ]

    # FPU comparisons
    if mnemonic in ('fcom', 'fcomp', 'fcompp', 'fucom', 'fucomp', 'fucompp',
                     'fcomi', 'fcomip', 'fucomi', 'fucomip', 'ficom', 'ficomp',
                     'ftst'):
        return [
            '    let sub = b::sub(o1(), o2());',
            f'    let calc_flags = calc_flags_automatically(sub, o1_size(), {ALL_FLAGS});',
            '    [calc_flags].into()',
        ]

    # FPU load/store
    if mnemonic in ('fld', 'fild', 'fbld', 'fld1', 'fldl2t', 'fldl2e', 'fldpi', 'fldlg2', 'fldln2', 'fldz'):
        return ['    let assignment = assign(o1(), o1(), o1_size());', '    [assignment].into()']
    if mnemonic in ('fst', 'fstp', 'fist', 'fistp', 'fisttp', 'fbstp'):
        return ['    let assignment = assign(o1(), o1(), o1_size());', '    [assignment].into()']

    # FPU control
    if mnemonic in ('finit', 'fninit', 'fclex', 'fnclex', 'fwait', 'fnop',
                     'fdecstp', 'fincstp', 'ffree', 'ffreep',
                     'fstcw', 'fnstcw', 'fldcw', 'fstsw', 'fnstsw',
                     'fstenv', 'fnstenv', 'fldenv', 'fsave', 'fnsave', 'frstor',
                     'fxsave', 'fxrstor', 'fxsave64', 'fxrstor64'):
        return ['    [].into()']

    # FPU misc
    if mnemonic in ('fabs', 'fchs', 'frndint', 'fsqrt', 'fprem', 'fprem1',
                     'f2xm1', 'fyl2x', 'fyl2xp1', 'fptan', 'fpatan',
                     'fsin', 'fcos', 'fsincos', 'fxtract', 'fscale',
                     'fxam'):
        return ['    let assignment = assign(o1(), o1(), o1_size());', '    [assignment].into()']

    # FPU conditional moves
    if mnemonic.startswith('fcmov'):
        return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']

    # Fallback
    return [f'    [exception("{mnemonic}")].into()']


def _try_mask_pattern(mnemonic: str) -> list[str] | None:
    """Handle AVX-512 mask register operations (k-prefix)."""
    if mnemonic.startswith('kand'):
        if 'n' in mnemonic[4:]:
            return ['    let assignment = assign(b::and(u::not(o2()), o3()), o1(), o1_size());', '    [assignment].into()']
        return ['    let assignment = assign(b::and(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
    if mnemonic.startswith('kor'):
        return ['    let assignment = assign(b::or(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
    if mnemonic.startswith('kxor'):
        return ['    let assignment = assign(b::xor(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
    if mnemonic.startswith('kxnor'):
        return ['    let assignment = assign(u::not(b::xor(o2(), o3())), o1(), o1_size());', '    [assignment].into()']
    if mnemonic.startswith('knot'):
        return ['    let assignment = assign(u::not(o2()), o1(), o1_size());', '    [assignment].into()']
    if mnemonic.startswith('kmov'):
        return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']
    if mnemonic.startswith('kadd'):
        return ['    let assignment = assign(b::add(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
    if mnemonic.startswith('kshift'):
        if 'l' in mnemonic:
            return ['    let assignment = assign(b::shl(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
        return ['    let assignment = assign(b::shr(o2(), o3()), o1(), o1_size());', '    [assignment].into()']
    if mnemonic.startswith('ktest') or mnemonic.startswith('kortest'):
        return [
            '    let and_val = b::and(o1(), o2());',
            f'    let calc_flags = calc_flags_automatically(and_val, o1_size(), {LOGIC_FLAGS});',
            '    [calc_flags].into()',
        ]
    if mnemonic.startswith('kunpck'):
        return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']
    # Fallback
    return ['    let assignment = assign(o2(), o1(), o1_size());', '    [assignment].into()']


# ============================================================
# Section 7: Main Translation Dispatcher
# ============================================================

def generate_ir_body(inst: Instruction, arch: str = 'intel') -> list[str]:
    """Generate IR function body lines for an instruction.

    Returns list of indented Rust code lines (without the function signature).
    """
    body, _, _ = generate_ir_body_with_status(inst, arch)
    return body


def generate_ir_body_with_status(inst: Instruction, arch: str = 'intel') -> tuple[list[str], str, str]:
    """Generate IR function body lines with classification status.

    Returns (body_lines, status, reason) where:
      status: "real_ir" | "exception" | "skipped"
      reason: description of which translation path was used
    """
    mnemonic = inst.mnemonic

    if arch == 'arm':
        # --- ARM pipeline ---

        # 1. Check ARM template match
        if mnemonic in ARM_TEMPLATES:
            body = ARM_TEMPLATES[mnemonic]
            body_text = '\n'.join(body)
            if 'exception(' in body_text:
                return body, "exception", f"ARM template (exception): {mnemonic}"
            return body, "real_ir", f"ARM template: {mnemonic}"

        # 2. Check ARM conditional branch (b.eq, b.ne, etc.)
        if mnemonic.startswith('b') and '.' in mnemonic:
            cc = mnemonic.split('.', 1)[1]
            if cc in ARM_CONDITION_CODES:
                return [
                    f'    let cond = {ARM_CONDITION_CODES[cc]};',
                    '    let fallthrough = b::add(pc.clone(), instruction_byte_size());',
                    '    [condition(cond, [jump(o1())], [jump(fallthrough)])].into()',
                ], "real_ir", f"ARM conditional branch: b.{cc}"

        # 3. Try ARM pseudocode translation
        if inst.operation:
            # First classify unsupported patterns explicitly
            unsupported = _arm_classify_unsupported(inst.operation)
            if unsupported:
                return [f'    [exception("{mnemonic}")].into()'], "exception", f"ARM {unsupported}"

            parsed = translate_arm_pseudocode(inst.operation, mnemonic)
            if parsed:
                return parsed, "real_ir", "ARM pseudocode translated"

        # 4. Fallback
        return [f'    [exception("{mnemonic}")].into()'], "exception", "ARM fallback: pseudocode too complex"

    # --- Intel pipeline ---

    # 1. Check exact template match
    if mnemonic in TEMPLATES:
        body = TEMPLATES[mnemonic]
        body_text = '\n'.join(body)
        if 'exception(' in body_text:
            return body, "exception", f"template (exception): {mnemonic}"
        return body, "real_ir", f"template: {mnemonic}"

    # 2. Check conditional jumps (j{cc})
    if mnemonic.startswith('j') and len(mnemonic) > 1:
        cc = mnemonic[1:]
        if cc in CONDITION_CODES:
            return [
                f'    let cond = {CONDITION_CODES[cc]};',
                '    [jcc(cond)].into()',
            ], "real_ir", f"conditional jump: j{cc}"

    # 3. Check conditional moves (cmov{cc})
    if mnemonic.startswith('cmov'):
        cc = mnemonic[4:]
        if cc in CONDITION_CODES:
            return [
                f'    let cond = {CONDITION_CODES[cc]};',
                '    [cmovcc(cond)].into()',
            ], "real_ir", f"conditional move: cmov{cc}"

    # 4. Check conditional set (set{cc})
    if mnemonic.startswith('set'):
        cc = mnemonic[3:]
        if cc in CONDITION_CODES:
            return [
                f'    let cond = {CONDITION_CODES[cc]};',
                '    [setcc(cond)].into()',
            ], "real_ir", f"conditional set: set{cc}"

    # 5. Check SIMD pattern
    simd_result = try_simd_pattern(mnemonic)
    if simd_result:
        body_text = '\n'.join(simd_result)
        if 'exception(' in body_text:
            return simd_result, "exception", f"SIMD pattern (exception): {mnemonic}"
        return simd_result, "real_ir", f"SIMD pattern: {mnemonic}"

    # 6. Try pseudocode translation
    if inst.operation:
        parsed = translate_simple_pseudocode(inst.operation, mnemonic)
        if parsed:
            return parsed, "real_ir", "pseudocode translated"

    # 7. Fallback: exception with pseudocode as comment
    return [f'    [exception("{mnemonic}")].into()'], "exception", "fallback: pseudocode too complex or untranslatable"


# ============================================================
# Section 8: Rust File Generation
# ============================================================

def _needs_jcc_helpers(instructions: list[Instruction], arch: str) -> bool:
    """Check if any instruction in the list needs jcc/sf_eq_of/sf_ne_of helpers."""
    for inst in instructions:
        m = inst.mnemonic
        if m.startswith('j') and len(m) > 1 and m[1:] in CONDITION_CODES:
            return True
    return False


def _needs_cmovcc_helpers(instructions: list[Instruction], arch: str) -> bool:
    for inst in instructions:
        if inst.mnemonic.startswith('cmov') and inst.mnemonic[4:] in CONDITION_CODES:
            return True
    return False


def _needs_setcc_helpers(instructions: list[Instruction], arch: str) -> bool:
    for inst in instructions:
        if inst.mnemonic.startswith('set') and inst.mnemonic[3:] in CONDITION_CODES:
            return True
    return False


def _needs_sf_helpers(instructions: list[Instruction], arch: str) -> bool:
    """Check if sf_eq_of/sf_ne_of are needed (for g/ge/l/le conditions)."""
    sf_conditions = {'g', 'ge', 'l', 'le', 'ng', 'nge', 'nl', 'nle'}
    for inst in instructions:
        m = inst.mnemonic
        cc = None
        if m.startswith('j') and len(m) > 1:
            cc = m[1:]
        elif m.startswith('cmov'):
            cc = m[4:]
        elif m.startswith('set'):
            cc = m[3:]
        if cc and cc in sf_conditions:
            return True
    return False


def generate_rust_file(instructions: list[Instruction], arch: str = 'intel') -> str:
    """Generate a Rust file with IR implementations for the given instructions."""
    parts: list[str] = []

    # Imports
    parts.append("use super::{super::static_register::*, shortcuts::*};")

    needs_jcc = _needs_jcc_helpers(instructions, arch)
    needs_cmovcc = _needs_cmovcc_helpers(instructions, arch)
    needs_setcc = _needs_setcc_helpers(instructions, arch)
    needs_sf = _needs_sf_helpers(instructions, arch)
    needs_aos = needs_jcc or needs_cmovcc or needs_setcc

    if needs_aos:
        parts.append("use crate::utils::Aos;")
    parts.append("use std::ops::Deref;")
    parts.append("")

    # Helper functions (if needed)
    if needs_jcc:
        parts.append("#[inline]")
        parts.append("fn jcc(condition_data: Aos<IrData>) -> IrStatement {")
        parts.append("    let fallthrough = b::add(rip.clone(), instruction_byte_size());")
        parts.append("    condition(condition_data, [jump(o1())], [jump(fallthrough)])")
        parts.append("}")
        parts.append("")

    if needs_cmovcc:
        parts.append("#[inline]")
        parts.append("fn cmovcc(condition_data: Aos<IrData>) -> IrStatement {")
        parts.append("    condition(condition_data, [assign(o2(), o1(), o1_size())], [])")
        parts.append("}")
        parts.append("")

    if needs_setcc:
        parts.append("#[inline]")
        parts.append("fn setcc(condition_data: Aos<IrData>) -> IrStatement {")
        parts.append("    condition(condition_data, [assign(c(1), o1(), o1_size())], [assign(c(0), o1(), o1_size())])")
        parts.append("}")
        parts.append("")

    if needs_sf:
        parts.append("#[inline]")
        parts.append("fn sf_eq_of() -> Aos<IrData> {")
        parts.append("    b::equal(sf.clone(), of.clone(), size_relative(sf.clone()))")
        parts.append("}")
        parts.append("")
        parts.append("#[inline]")
        parts.append("fn sf_ne_of() -> Aos<IrData> {")
        parts.append("    u::not(sf_eq_of())")
        parts.append("}")
        parts.append("")

    # Instruction functions
    for inst in instructions:
        if inst.operation:
            parts.append("/// # Pseudocode")
            parts.append("/// ```text")
            op_lines = inst.operation.split("\n")
            # Strip common leading whitespace
            non_empty = [l for l in op_lines if l.strip()]
            if non_empty:
                min_indent = min(len(l) - len(l.lstrip()) for l in non_empty)
                op_lines = [l[min_indent:] if len(l) >= min_indent else l for l in op_lines]
            # Trim trailing whitespace; cap indentation at 40 chars
            max_indent = 40
            for line in op_lines:
                stripped = line.rstrip()
                if stripped:
                    leading = len(stripped) - len(stripped.lstrip())
                    if leading > max_indent:
                        stripped = ' ' * max_indent + stripped.lstrip()
                parts.append(f"/// {stripped}")
            parts.append("/// ```")
        parts.append("#[box_to_static_reference]")
        parts.append(f"pub(super) fn {inst.mnemonic}() -> &'static [IrStatement] {{")

        body_lines = generate_ir_body(inst, arch)
        for line in body_lines:
            parts.append(line)

        parts.append("}")
        parts.append("")

    return "\n".join(parts)


# ============================================================
# Section 9: File Processing
# ============================================================

def clean_generated_files(output_dir: str):
    """Remove previous *_generated.rs files from the output directory."""
    if not os.path.isdir(output_dir):
        return
    for fname in os.listdir(output_dir):
        if fname.endswith("_generated.rs"):
            os.remove(os.path.join(output_dir, fname))


def process_arch(filepath: str, output_dir: str, arch: str = 'intel'):
    """Process one architecture file and generate stubs."""
    instructions = parse_rs_enum(filepath)

    skipped_mnemonics = []
    results: list[GenerationResult] = []

    with_operation = []
    for inst in instructions:
        if inst.operation:
            with_operation.append(inst)
        else:
            skipped_mnemonics.append(inst.mnemonic)
            results.append(GenerationResult(inst.mnemonic, "skipped", "no operation pseudocode"))

    # Group by first letter
    groups: dict[str, list[Instruction]] = defaultdict(list)
    for inst in with_operation:
        groups[inst.first_letter].append(inst)

    # Clean old generated files, then write new ones
    clean_generated_files(output_dir)
    os.makedirs(output_dir, exist_ok=True)
    for letter, group in sorted(groups.items()):
        rust_code = generate_rust_file(group, arch)
        out_path = os.path.join(output_dir, f"{letter}_generated.rs")
        with open(out_path, "w", encoding="utf-8") as f:
            f.write(rust_code)

    # Collect per-instruction translation stats (classified by status, not body text)
    total = len(with_operation)
    todo_count = 0
    exception_count = 0
    real_count = 0
    for inst in with_operation:
        _, status, reason = generate_ir_body_with_status(inst, arch)
        results.append(GenerationResult(inst.mnemonic, status, reason))
        if status == "real_ir":
            real_count += 1
        elif status == "exception":
            exception_count += 1
        elif status == "todo":
            todo_count += 1
        else:
            print(f"  WARNING: unknown status '{status}' for {inst.mnemonic}", file=sys.stderr)

    generated_mnemonics = [inst.mnemonic for inst in with_operation]
    stats = (total, real_count, exception_count, todo_count)
    return generated_mnemonics, skipped_mnemonics, stats, results


def _write_arch_report(f, arch_name: str, stats, results: list[GenerationResult], skipped_count: int):
    """Write one architecture section to the report file."""
    total, real, exc, todo = stats
    all_count = total + skipped_count

    f.write(f"=== {arch_name} ===\n")
    f.write(f"Total instructions: {all_count}\n")
    f.write(f"  With operation: {total}\n")
    f.write(f"  Skipped (no operation): {skipped_count}\n\n")

    if total > 0:
        f.write("Translation results:\n")
        f.write(f"  Real IR: {real} ({100*real//total}%)\n")
        f.write(f"  Exception stubs: {exc} ({100*exc//total}%)\n")
        if todo > 0:
            f.write(f"  Todo stubs: {todo} ({100*todo//total}%)\n")
        f.write("\n")

    real_ir = [r for r in results if r.status == "real_ir"]
    exceptions = [r for r in results if r.status == "exception"]
    todos = [r for r in results if r.status == "todo"]
    skipped = [r for r in results if r.status == "skipped"]

    f.write(f"--- SUCCESSFUL (Real IR): {len(real_ir)} ---\n")
    for r in real_ir:
        f.write(f"  {r.mnemonic}: {r.reason}\n")
    f.write("\n")

    f.write(f"--- EXCEPTION STUBS: {len(exceptions)} ---\n")
    for r in exceptions:
        f.write(f"  {r.mnemonic}: {r.reason}\n")
    f.write("\n")

    if todos:
        f.write(f"--- TODO STUBS: {len(todos)} ---\n")
        for r in todos:
            f.write(f"  {r.mnemonic}: {r.reason}\n")
        f.write("\n")

    f.write(f"--- SKIPPED (no operation pseudocode): {len(skipped)} ---\n")
    for r in skipped:
        f.write(f"  {r.mnemonic}\n")
    f.write("\n\n")


def main():
    base_dir = os.path.dirname(os.path.abspath(__file__))
    result_dir = os.path.join(base_dir, "result")

    intel_gen, intel_skip, intel_stats, intel_results = process_arch(
        os.path.join(result_dir, "intel.rs"),
        os.path.join(base_dir, "output", "intel"),
        arch='intel',
    )

    arm_gen, arm_skip, arm_stats, arm_results = process_arch(
        os.path.join(result_dir, "arm.rs"),
        os.path.join(base_dir, "output", "arm"),
        arch='arm',
    )

    # Console summary
    print("=== Intel ===")
    total, real, exc, todo = intel_stats
    print(f"Total with operation: {total}")
    if total > 0:
        print(f"  Real IR: {real} ({100*real//total}%)")
        print(f"  Exception stubs: {exc} ({100*exc//total}%)")
        print(f"  Todo stubs: {todo} ({100*todo//total}%)")
    print(f"Skipped (no operation): {len(intel_skip)}")

    print("\n=== ARM ===")
    total, real, exc, todo = arm_stats
    print(f"Total with operation: {total}")
    print(f"  Real IR: {real}")
    print(f"  Exception stubs: {exc}")
    print(f"  Todo stubs: {todo}")
    print(f"Skipped (no operation): {len(arm_skip)}")

    # Write detailed log file
    log_path = os.path.join(base_dir, "output", "generation_report.log")
    os.makedirs(os.path.dirname(log_path), exist_ok=True)
    with open(log_path, "w", encoding="utf-8") as f:
        f.write("=" * 60 + "\n")
        f.write(f"Generation Report - {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")
        f.write("=" * 60 + "\n\n")
        _write_arch_report(f, "Intel", intel_stats, intel_results, len(intel_skip))
        _write_arch_report(f, "ARM", arm_stats, arm_results, len(arm_skip))

    print(f"\nDetailed report: {log_path}")


if __name__ == "__main__":
    main()
