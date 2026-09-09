# Performance Plan (đợt A)

Baseline: `fix-hardening-plan` sau Phase 1–4 — clippy pedantic+nursery gần sạch
(1 warning accepted), test xanh, chưa có bench nào, service chưa có span riêng.

Quy tắc: benchmark trước khi optimize (mục nào bench không cải thiện đo được thì bỏ);
không đổi behavior nghiệp vụ; mỗi phase fmt + clippy + test xanh mới commit.

## Phase 5 — Đo trước (không đổi behavior)

Branch: `phase5-perf-measure` (từ tip `fix-hardening-plan`). Status: **DONE.**

- [x] `criterion` dev-dep (console stats, không plots) + `benches/`: `debt_engine`
      (n = 5/20/100), `split_validation` (đúng/sai), `share_grouping` (sang Phase 7).
- [x] Span `get_group_summary`, `DebtEngine` (skip arg nặng, chỉ ghi count),
      `check_rate_limit` (skip key chứa IP), `RedisConfig::connect` (skip URL vì
      có thể chứa password, chỉ ghi timeout).
- [x] Baseline: net_balances 0.79/6.9/142µs; simplify 105/237/951ns;
      split validation 4–11ns (không đáng optimize thêm).

## Phase 6 — I/O roundtrips (giảm số lần gọi DB/Redis)

Branch: `phase6-io-roundtrips` (stack trên tip Phase 5). Status: **DONE.**

- [x] Rate limiter 1 Lua script (`INCR` + `EXPIRE` nếu == 1); mixed gộp 2 keys
      vào 1 script → 4 RTT còn 1, atomic, hết race rò rỉ key. `redis::Script` tự
      fallback `EVAL`. Fail-open giữ nguyên. Không verify live được (chưa có
      Redis local) — logic script đơn giản, chờ verify tay khi có Redis.
- [x] `COUNT(*) OVER()` gộp COUNT + SELECT thành 1 query cho expense và settlement
      list (2 RTT còn 1). Decode qua `FromRow::from_row` (bỏ qua cột thừa), không
      đổi signature repo.
- [x] Cache membership `group:members:{id}` (id + role, TTL 60s): `ensure_*` ở cả
      3 services dùng chung helper; xóa exact khi add member/delete group.
      Trade-off đã chốt: staleness tối đa 60s.
- [x] `CacheStore::delete_many` (Redis pipeline, backend khác loop); các vòng xóa
      session (`sign_out`, `change/reset password`, đuổi session cũ) gộp batch.
- [x] Test: membership hit + invalidation proof, `delete_many` unit, full suite xanh.

## Phase 7 — Alloc, hotpath, query hygiene

Branch: `phase7-alloc-hotpath` (stack trên tip Phase 6). Status: **DONE.**

- [x] Mapper by-value (hết clone từng field ở call site sở hữu entity). Ngoại lệ đúng
      theo clippy: mapper toàn field `Copy` giữ `&` + `const` (không có gì để move).
- [x] Gom shares `HashMap` O(E+S): 100e_10s 88.8µs → 39.8µs (2.2x); small-n chậm
      hơn ~1µs (không đáng kể cạnh RTT DB ms) — giữ vì scale tuyến tính.
- [x] `hashbrown` 2 map nóng: net_balances n=100 142µs → 45µs (3.1x).
- [x] Bỏ theo đúng luật bench: `SmallVec` (simplify regress 25–50%), `#[inline]`
      (không delta — LLVM tự inline ở opt level).
- [x] `SELECT *` → liệt kê cột ở mọi query nóng (trừ `UNNEST` và `RETURNING *`
      vốn cần full row).
- [x] Test: unit test helper gom shares, full suite xanh, response JSON không đổi.

## Follow-up (chưa duyệt scope, không làm đợt này)

- Metrics latency (`metrics` crate + endpoint Prometheus) theo mục Observability AGENTS.md.
- Pool size production (`DB_MAX_CONNECTIONS`) — đã chỉnh được qua env, không đổi code.
- Benchmark endpoint end-to-end (oha/k6) khi có môi trường staging.

## Quyết định đã chốt

- Bench trước, số liệu quyết định giữ/bỏ từng mục (AGENTS.md: benchmark trước optimize).
- Membership cache: exact invalidation + TTL 60s dự phòng; staleness tối đa 60s được chấp nhận.
- Lua script OK trên Redis 6+ (đúng yêu cầu README); `redis::Script` tự fallback `EVAL`.
- `hashbrown` direct dep OK (đã nằm sẵn trong dependency tree).
- Không optimize mù: mục nào bench không cải thiện thì bỏ, ghi lý do vào commit.
