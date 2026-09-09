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

Branch: `phase3-docs` (stacked on Phase 2 tip `1dc2d09`). Status: **DONE — merged into
`fix-hardening-plan` as `d33ffd8` (code commit `55f967f`).**

Docs style (quy tắc chốt): **tiếng Việt, ngắn gọn 1–2 dòng, đủ hiểu** — không verbose.
Chỉ docs ở pub fn chuẩn (public API: handlers, services, repos, config, utils dùng chung);
không docs tràn lan mọi hàm nội bộ. Mỗi docs nói: hàm làm gì + `# Errors` khi trả `Result`.

- [x] `///` + `# Errors` cho toàn bộ pub fn (handlers, services, repos, mappers,
      middleware, utils, config, errors, responses, state); docs tiếng Anh cũ
      (config builders, validator, sqlx) rút gọn sang tiếng Việt, giữ bảng env.
- [x] `missing_errors_doc` về 0; `cargo doc --no-deps` sạch.
- [x] Bỏ `async` không justify: `DebtEngine` thuần CPU → sync trả giá trị trực tiếp
      (caller + test gọn theo); `deliver` stub → sync, bỏ `Result` giả
      (`unnecessary_wraps`).
- [x] Giữ `async` có justify bằng comment: `require_auth`/`from_request_parts`
      (axum bắt buộc), `hash_*` (`spawn_blocking`), handlers/services (I/O).
- [x] Phụ: `MAX_SESSIONS_PER_USER` vào constants (bắt được magic `5`),
      `hex::encode` → `pub(super)`.

Acceptance: `cargo fmt --check`, `cargo test` green, `cargo doc` sạch.

Accepted warning duy nhất (có lý do, không suppress vì AGENTS.md cấm cờ ở production):
`unused_async_trait_impl` ở `FromRequestParts::from_request_parts` — axum ép signature
`async`, body không `.await`; đã ghi justification comment tại code.

## Phase 4 — Test layout and async proofs

Branch: `phase4-tests-layout` (stacked on Phase 3 tip `55f967f`). Status: **DONE —
merged into `fix-hardening-plan` as `3cdc356` (code commit `4417740`).**

- [x] Restructure `tests/` đúng layout AGENTS.md: `tests/unit` (debt_engine,
      split_strategy, i18n, jwt — 14 cases), `tests/integration` (4 service tests +
      1 api test, 2 cases `#[ignore]` có lý do), `tests/system` (mới).
- [x] Thêm system smoke test `redis_startup_test`: URL sai → `InvalidUrl` ngay,
      host unreachable → lỗi trong timeout 1s (assert < 10s, pass cả khi sandbox
      không mạng vì vẫn là `Err`).
- [x] `unwrap()`/`expect()` chỉ còn trong tests (được phép); 2 cases `#[ignore]`
      giữ nguyên lý do (cần Postgres thật).

Acceptance: `cargo test` green toàn bộ layout mới (36 lib + 14 unit + 5 integration
+ 2 system), `cargo fmt --check` OK.

## Review Checklist (trả lời chung cho cả 4 phases)

- Race condition: không — fakes dùng `tokio::Mutex` đúng cách, guard đã scope hẹp (Phase 2).
- Deadlock: không — `find_all_by_user` mock khóa tuần tự từng lock trong block riêng, không
  lồng lock; production không giữ lock qua `.await` (không có `std::Mutex` trong async).
- Giữ lock qua await: không (production); test fakes đã scope guard (lint
  `significant_drop_tightening` sạch).
- Task leak: không — `Mailer` worker chạy vòng `recv()` đến khi channel đóng; không
  `mem::forget`, `JoinHandle` nào bị bỏ quên (spawn duy nhất có vòng đóng rõ ràng).
- Cancellation-safe: có — mọi `.await` trong handlers/services đều trong hàm trả `Result`,
  drop giữa chừng chỉ hủy request đó, không để lại state dở (DB ops nằm trong transaction).
- Unnecessary clone/Arc: đã quét — `redundant_clone` sạch; `Arc` còn lại đều shared
  ownership thật (`AppState`, services, cache, pool).
- Allocation trong hot path: không phát hiện mới — `format!` cho cache key là per-request
  (chấp nhận được, chưa benchmark nên chưa optimize theo quy tắc "benchmark trước").
- Benchmark: chưa — không có thay đổi nào thuộc hot path đòi benchmark (ghi nhận follow-up).
- Test edge cases: có — strategy (uneven/shortfall/single-share), Redis fail-fast
  (invalid URL + unreachable), OTP/enumeration paths giữ nguyên.
- Clippy: `cargo clippy --all-targets` chỉ còn 1 warning accepted có lý do
  (`unused_async_trait_impl` ở axum `FromRequestParts` — framework ép signature).
- Docs: `cargo doc --no-deps` sạch, style Việt ngắn gọn theo quy tắc đã chốt.

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
