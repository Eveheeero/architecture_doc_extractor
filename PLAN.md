# Plan

  The ARM ?_generated.rs path is already structurally in place in this repo. The shortest path is not a new generator, but finishing the existing ARM branch in generate_fireman_stubs.py so
  it can consume result/arm.rs and emit stable output/arm/{letter}_generated.rs. The Fireman repo confirms the intended output shape: grouped-by-letter files, doc-commented pseudocode,
  #[box_to_static_reference], and a template -> conditional -> pseudocode translator -> exception fallback pipeline.

## Intent Summary

- Most likely path: extend ARM templates in the current generator, not the Rust extractor.
- Main risk: over-expanding the translator into unsupported SIMD, memory-atomic, or crypto pseudocode.
- Best iteration boundary: template coverage first, translator tweaks second, validation last.

## Assumptions

- Target output is local output/arm/*_generated.rs, not direct write-back into Fireman.
- Existing ARM extraction in src/arm/arm.rs and src/arm/v1/mod.rs is already good enough as pseudocode input.
- Unsupported families such as SIMD loops and complex memory semantics should remain exception(...) for now.

## Requirements

- Preserve the current ARM document-extraction flow:
    src/main.rs -> src/arm/arm.rs -> result/arm.rs -> generate_fireman_stubs.py -> output/arm/*_generated.rs.
- Keep generated ARM functions aligned with Fireman’s output convention.
- Improve real IR coverage incrementally without breaking existing exception fallback behavior.
- Support repeated runs with clear stop points after each batch of coverage.

## Scope

- In:
  - ARM template additions in the Python generator
  - Small ARM translator improvements
  - Validation updates for newly supported patterns
  - Regeneration of output/arm/*_generated.rs
- Out:
  - Reworking the Rust XML extractor
  - Full SIMD/SVE loop semantics
  - Complex Mem[], exclusive monitor, AES/SHA/CRC/PAC semantics
  - Structural changes to Fireman itself

## Files and Entry Points

- src/arm/arm.rs
  - Produces result/arm.rs from ARM XML data.
- src/arm/result.rs
  - Defines the extracted ARM instruction model and controls how ## Operation pseudocode appears in markdown/output.
- generate_fireman_stubs.py
  - Contains the real implementation target:
        ARM register maps, condition codes, templates, unsupported classifier, pseudocode translation, and file emission.
- validate_ir_semantics.py
  - Semantic guardrail for generated ARM functions.
- PLAN.md
  - Already contains a useful tactical backlog for ARM exception reduction; use it as the initial work queue.

## Data Model / API Changes

- No Rust-side data model change is required for the first completion pass.
- The operative contract is the format of result/arm.rs:
    enum variants with doc comments and a recoverable ## Operation code block.
- Generated output contract should remain:
  - grouped by first mnemonic letter
  - one pub(super) fn mnemonic() -> &'static [IrStatement]
  - pseudocode doc block retained above each function
  - exception fallback preserved when translation is unsafe

## Action Items

  [x] Verify that the first implementation target is the existing ARM generator path in generate_fireman_stubs.py, not the Rust extractor.

  [x] Implement a first batch of deterministic ARM templates, using the categories already identified in PLAN.md: branch families (cbz, cbnz, tbz, tbnz), direct PSTATE rewrites (axflag,
  xaflag), simple arithmetic (subp, subps), and low-risk load/store aliases.

  [x] Keep Fireman parity at the output boundary by preserving doc-commented pseudocode and letter-bucketed file generation in generate_fireman_stubs.py.

  [x] After template coverage, make only narrow translator improvements:

- relax ARM flat-parser handling for skipped declarations
- widen line-count tolerance for short multi-line pseudocode
- add safe skip patterns for enablement checks
- avoid broad removal of unsupported-pattern guards

  [x] Regenerate output/arm/*_generated.rs and inspect the delta by category: newly real IR, still exception, skipped.

  [x] Extend validate_ir_semantics.py only for the new supported ARM shapes so validation remains aligned with actual generator capability.

  [x] Leave SIMD loop, Elem[], atomic Mem[], and crypto/SVE families as explicit follow-up batches rather than mixing them into the first completion pass.

  [x] Land the next scalar-only ARM cleanup batch by converting `rmif` from an exception stub into a real IR template and validating the result.

  [x] Do not auto-expand into the remaining SIMD/vector-loop, Elem[], Mem[], crypto, or system-heavy backlog without an explicit scope change.

  [x] Broaden into the first higher-risk scalar bitfield batch by landing `ubfm`, `sbfm`, and `bfm` with real rotate/mask/sign-extension or destination-preserving merge IR and validator coverage.

  [x] Add alias-subset semantic validation for the safe scalar bitfield aliases `ubfiz`, `sbfiz`, `bfi`, and `bfxil` at the canonical `ubfm` / `sbfm` / `bfm` structural level.

  [x] Extend alias-level semantic validation to `ubfx` and `sbfx` only by checking the canonical `ubfm` / `sbfm` non-wrap extract mapping, with sign-extension enforced only for `sbfx`.

## Progress Update

- First ARM execution batch is complete in the current worktree:
  - generator path confirmed as the implementation target
  - branch families (`cbz`, `cbnz`, `tbz`, `tbnz`) now emit real IR
  - direct PSTATE rewrites (`axflag`, `xaflag`) now emit real IR
  - simple arithmetic (`subp`, `subps`) now emit real IR
  - low-risk load/store aliases were added and validated
- The ARM extractor panic was fixed in `src/arm/result.rs` by guarding malformed bitfield-width subtraction while rendering markdown labels.
- Current post-regeneration baseline is now:
  - ARM total generated functions: 704
  - ARM real IR: 228
  - ARM correct: 194
  - ARM acceptable: 34
  - ARM wrong: 0
  - ARM exception stubs: 476
- The remaining scalar trap/debug/system mnemonics (`brk`, `dcps1/2/3`, `drps`, `hlt`, `hvc`, `svc`, `smc`) were reviewed and intentionally left as exception stubs.
  - Reason: this IR surface has no honest trap/syscall/debug primitive, so converting them to “real IR” would hide control-flow side effects rather than model them.
- The scalar bitfield family is no longer fully deferred:
  - `ubfm`, `sbfm`, and `bfm` now emit real IR through explicit wrap/non-wrap handling plus sign-extension or destination-preserving merge semantics as appropriate.
  - The safe alias subset (`ubfiz`, `sbfiz`, `bfi`, `bfxil`) is now covered by semantic validation at the canonical bitfield structure level:
    `ubfiz` / `sbfiz` must keep the wrap-path `insert_width` + `datasize - immr` remapping, and `bfi` / `bfxil` must preserve the destination merge while selecting the wrap/insert vs non-wrap/extract branch correctly.
  - `ubfx` and `sbfx` are now validated only at the alias/semantic guardrail layer by asserting the non-wrap extract mapping on the canonical `ubfm` / `sbfm` bodies, while direct alias-aware emission is still deferred.
- ARM extraction now preserves alias preference conditions from XML on canonical instructions.
  - `result/arm.rs` and `result/arm/*.md` include `Preferred when ...` conditions for aliases, sourced from `<aliaspref>` in the original ARM XML.
  - This is metadata-only groundwork; alias-aware emission is still deferred until the generator is taught to consume the new structured alias conditions safely.
- The Python generator parser now preserves alias metadata from the extracted Rust docs.
  - `generate_fireman_stubs.py` carries parsed `## Aliases` entries alongside canonical ARM instructions instead of discarding them.
  - Alias-aware emission is still deferred, but the generator input now has the metadata needed for that next phase.

## Suggested Iteration Boundary

  First execution completed with:

  1. Template additions for the low-risk ARM families.
  2. Minimal translator fixes.
  3. A full extractor rerun after the `src/arm/result.rs` panic fix.
  4. One regeneration pass of output/arm/*_generated.rs.
  5. A scalar-only `rmif` follow-up batch.
6. A refreshed ARM coverage report (704 total generated ARM functions, 228 real IR functions, 476 exception stubs, 194 correct, 34 acceptable, 0 wrong after the regenerated alias-validation pass).

- Run validate_ir_semantics.py against the generated ARM files.
- Run the ARM-specific output validator in validate_arm_output.py.
- Compare generated function shape against Fireman’s reference style from fireball/src/arch/arm/instruction_analyze/{a..z}_generated.rs in the reference repo.

- Current validated commands:
- `cargo run --bin architecture_doc_extractor_cli`
- `python3 generate_fireman_stubs.py`
- `python3 validate_ir_semantics.py`
- `python3 validate_arm_output.py`

## Risks and Edge Cases

- result/arm.rs is mnemonic-aggregated, so some per-encoding semantic detail may already be collapsed before generation.
- Condition-code instructions and PSTATE rewrites are sensitive to evaluation order; templates must snapshot original flag values before mutation where required.
- Load/store aliases are only safe if the disassembler layer already resolves memory operands into the expected IR operand shape.
- Broadening the generic ARM pseudocode translator too aggressively will create false “real IR” coverage and hurt validation quality.
- The biggest unsupported backlog is still SIMD/vector loops and complex memory semantics; those should remain exception stubs until there is a real IR representation strategy.
- `rmif` is a good follow-up because it is scalar-only and uses existing rotate/condition/PSTATE assignment primitives; most remaining exception stubs do not share that property.
- The next honest ARM expansion is still alias-aware emission, not broader alias coverage in validation.
  - The scalar bitfield alias guardrail now covers `ubfiz` / `sbfiz` / `bfi` / `bfxil` plus `ubfx` / `sbfx`, but `ubfx` / `sbfx` are still validated against the canonical non-wrap extract form rather than a dedicated emitted alias body.
  - Do not broaden the validator or generator into `lsl` / `lsr` / `asr` or `uxt*` / `sxt*` aliases in this batch.

## Open Questions

- Should the next implementation run stop at local output/arm/*_generated.rs, or also prepare a sync step into the Fireman tree?
- Is the goal “maximize safe real IR coverage” or “mirror Fireman categories exactly even when behavior is simplified”?
- If a mnemonic has mixed-simple and mixed-complex encodings, do you want the first pass to prefer a conservative exception stub for the whole mnemonic or a best-effort simplified
    template?
- Which explicitly higher-risk batch should come next: the remaining bitfield aliases, scalar/system control instructions, or a deliberate SIMD/vector tranche?
- If trap/debug/system instructions should stop being exception stubs, do you want a new IR primitive added for traps/syscalls/debug halts first, or should they remain deferred?

### Critical Files for Implementation

- /home/mull/temp/architecture_doc_extractor/PLAN.md - Existing ARM backlog with the most concrete first-batch coverage targets and expected gains.
