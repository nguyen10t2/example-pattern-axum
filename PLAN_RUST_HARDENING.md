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

Branch: `phase2-pedantic-nursery` (stacked on Phase 1 commit `0a578db` to avoid
re-doing Phase-1 fixes with `clippy --fix`). Status: **DONE — merged into
`fix-hardening-plan` as `bb8753a` (code commit `1dc2d09`).**

- [x] `[lints.clippy] pedantic + nursery = "warn"` in `Cargo.toml` — plain
      `cargo clippy` now enforces the level.
- [x] `clippy --fix` reviewed hunk by hunk: `#[must_use]` hàng loạt, `const fn`
      cho builders/constructors/mappers, `Self::`, `i64::from`, backticks.
- [x] Casts: `cast_signed()` cho `u64→i64`, `try_from` + `map_err`/`unwrap_or_default`
      (không `unwrap` production), ceil float → `(total-1)/limit+1` chính xác tuyệt đối.
- [x] Scope `MutexGuard` trong test fakes (block + `drop`显式), `i18n::t` generic
      `BuildHasher`, drop bound `Serialize` thừa, `std::future::ready` cho MemoryCache,
      `604800` → const.

Acceptance: `cargo fmt --check`, `cargo clippy --all-targets` chỉ còn 2 nhóm thuộc
Phase 3 (`missing_errors_doc`, `unused_async`), `cargo test` green.

Lessons:
- `div_ceil` trên `i64` vẫn unstable (chỉ bản unsigned stable) — dùng công thức nguyên.
- `fn` không có default type param trên stable — xem Phase 1 lessons.
- Sync-impl-of-async-trait: `std::future::ready` thoát cả `unused_async_trait_impl`
  lẫn `manual_async_fn`, zero-alloc (xem quyết định đã chốt ở cuối file).

## Phase 3 — Documentation coverage

Branch: `phase3-docs` (stack on Phase 2 tip to avoid conflicts).

Docs style (quy tắc chốt): **tiếng Việt, ngắn gọn 1–2 dòng, đủ hiểu** — không verbose.
Chỉ docs ở pub fn chuẩn (public API: handlers, services, repos, config, utils dùng chung);
không docs tràn lan mọi hàm nội bộ. Mỗi docs nói: hàm làm gì + `# Errors` khi trả `Result`.

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
