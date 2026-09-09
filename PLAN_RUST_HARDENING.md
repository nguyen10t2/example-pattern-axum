# Rust Hardening Plan (AGENTS.md Audit Follow-up)

Audit date: 2026-09-09. Baseline: `master@d86d7a1` — default `cargo clippy` 0 warnings,
`cargo test` green. Pedantic + nursery: ~192 lib warnings (levels never enabled before).

Conventions: `cargo fmt`, `cargo clippy --all-targets`, `cargo test` green before every
commit. No `unwrap()`/`expect()` in production code. No clippy suppression flags in
production code.

## Phase 1 — Quick wins (low risk, no behavior change)

Branch: `phase1-clippy-quick-wins` (from `master`). Status: **DONE — merged into
`fix-hardening-plan` as `d14f97e` (code commit `0a578db`).**

- [x] Fix `redundant_clone` in `src/state.rs` — only repos moved on last use
      (`user_repo`, `expense_repo`, `settlement_repo`, `group_repo`,
      `connection_manager`); `cache`/`pool` clones kept (moved into `Self` later).
- [x] Merge identical match arms in `src/errors/business.rs` with `|` patterns
      (grouped by status code; merged shared `error_codes` arms).
- [x] Inline format args (`format!("{x}")`), `redundant_closure` ×3
      (`Cookie::value`, `Option::cloned`), single-pattern `match` → `if let` ×1.
- [x] `map_or_else` ×2, `String::new()` ×1, `clone_from` ×3, `HashMap` hasher
      generality ×1 (generic `S: BuildHasher` + turbofish `None` at 8 call sites).
- [x] `ignored_unit_patterns` in `main.rs` (`() = ctrl_c => ()`), `needless_raw_string_hashes`
      in `groups/pg.rs`.

Acceptance: `cargo fmt --check`, `cargo clippy --all-targets` (default) 0 warnings,
`cargo test` green. All 12 Phase-1 lint groups verified at 0 via clippy JSON report.

Lessons (do not repeat):
- Inline format args accept identifiers only — `{data.group_id}` is a compile error;
  that is why clippy never flagged those lines.
- Check which clones are last-use per variable, not per line (`cache`/`pool` move
  into `Self`; E0382 catches mistakes).
- Stable `fn` cannot have default type params — generic `BuildHasher` requires
  turbofish `None` at every `None` call site.

## Phase 2 — Enforce pedantic + nursery, fix mechanical lints

Branch: `phase2-pedantic-nursery` (from `master`).

- [ ] Add `[lints.clippy] pedantic = "warn", nursery = "warn"` to `Cargo.toml`
      (plus `missing_errors_doc`, `missing_panics_doc` if desired) so CI locks the level.
- [ ] Run `cargo clippy --fix`, review the diff hunk by hunk. Expected bulk:
  - `#[must_use]` ×36 (methods + functions).
  - `const fn` candidates ×21 (config builders and pure helpers).
  - Cast lints (`usize↔i64`, `u64→i64` e.g. `REFRESH_TOKEN_EXPIRATION as i64`,
    `count as i64` in `strategy.rs`): prefer `cast_signed()` / `From` where genuinely
    infallible; keep `as` only with a comment proving the range.
  - `Self` repetition ×6, digit separators (`604_800`) ×2, doc backticks ×1.
- [ ] Scope `MutexGuard` temporaries in `tests/common/mod.rs` ×5
      (`significant_drop_tightening` — also satisfies "no lock across `.await`").

Acceptance: `cargo clippy --all-targets -- -W clippy::pedantic -W clippy::nursery`
0 warnings, `cargo test` green.

## Phase 3 — Documentation coverage

Branch: `phase3-docs` (from `master`).

- [ ] Add `///` doc comments to the 123 public functions missing them
      (handlers, mappers, service methods, `pg::new()` constructors).
      Work module by module: `domain/users`, `domain/groups`, `domain/expenses`,
      `domain/settlements`, `middleware`, `utils`, `config`, `responses`.
- [ ] Add `# Errors` sections to the 74 `Result`-returning functions missing them
      (pedantic `missing_errors_doc`).
- [ ] Justify `async` where non-obvious (AGENTS.md: every async fn must justify why):
  - `middleware/auth.rs::require_auth` — MUST stay `async` (axum `from_fn` requires
    it); justify with a comment, do NOT remove.
  - `utils/email.rs::deliver` — stub awaiting SMTP/provider integration; justify
    with a comment (or drop `async` until the I/O lands — team decision).
  - `domain/debt_engine.rs:143,147` — remove `async` (pure computation, no `.await`).

Acceptance: `cargo doc --no-deps` clean under `-D missing-docs -D clippy::missing_errors_doc`
(or equivalent `RUSTDOCFLAGS`), `cargo test` green.

## Phase 4 — Test layout and async proofs

Branch: `phase4-tests-layout` (from `master`).

- [ ] Restructure `tests/` to AGENTS.md layout: `tests/unit`, `tests/integration`,
      `tests/system` (current: `api`, `common`, `core`, `services`, `utils`).
      Proposed mapping: `core` + unit `mod tests` → `unit`; `services` + `api` →
      `integration`; add smoke `system` test (boot + `/` route + Redis-down fail-fast).
- [ ] Keep `unwrap()`/`expect()` in tests (explicitly allowed) but ensure every
      ignored test documents why (`#[ignore]` + reason, as existing DB tests do).
- [ ] Answer the Review Checklist for each phase and paste it into the PR description:
      race/deadlock, lock-across-await, task leak, cancellation-safety, unnecessary
      clone/Arc, hot-path allocation, edge-case tests, clippy, docs.

Acceptance: `cargo test` green including new layout, Review Checklist answered per phase.

## Decisions already locked (do not revisit without new evidence)

- `async-trait` stays for generic `Executor`-based repository traits: native `async fn`
  breaks axum `Handler` `Send` bounds via RPITIT lifetime capture
  (rust-lang/rust#100013, verified by failed build). `CacheStore` uses native async
  with explicit `impl Future + Send` over the `Cache` enum (static dispatch, no `dyn`).
- `EQUAL` split = floor/ceil distribution check on top of sum check (`p1-equal-split-strict`,
  merged). `EXACT`/`PERCENTAGE` semantics unchanged.
- `chrono` for wall-clock timestamps, `std::time`/`tokio::time` for durations/timeouts,
  `time` crate only at the cookie API boundary.
- Shared defaults live in `src/config/constants.rs` (single source of truth).
