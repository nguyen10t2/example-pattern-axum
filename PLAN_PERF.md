# Performance Plan (đợt A)

Baseline: `fix-hardening-plan` sau Phase 1–4 — clippy pedantic+nursery gần sạch
(1 warning accepted), test xanh, chưa có bench nào, service chưa có span riêng.

Quy tắc: benchmark trước khi optimize (mục nào bench không cải thiện đo được thì bỏ);
không đổi behavior nghiệp vụ; mỗi phase fmt + clippy + test xanh mới commit.

## Phase 5 — Đo trước (không đổi behavior)

Branch: `phase5-perf-measure` (stack trên tip `fix-hardening-plan`).

- [ ] Thêm `criterion` dev-dependency + `benches/`:
  - `debt_engine`: `calculate_net_balances` + `simplify_debts` với nhóm n = 5/20/100.
  - `split_validation`: equal/exact/percentage (input hợp lệ + không hợp lệ).
  - `share_grouping`: gom shares theo expense — bản `filter` hiện tại (baseline cho Phase 7).
- [ ] Gắn `tracing::instrument` (skip arg nặng) vào: `get_group_summary`,
      `DebtEngine` 2 hàm, `check_rate_limit`, `RedisConfig::connect`.
- [ ] Chỉ bench hàm thuần (không cần DB/Redis thật); ghi số baseline vào commit message.

Acceptance: `cargo bench --no-run` pass, `cargo test` green, có số baseline.

## Phase 6 — I/O roundtrips (giảm số lần gọi DB/Redis)

Branch: `phase6-io-roundtrips` (stack trên tip Phase 5).

- [ ] Rate limiter bằng 1 Lua script (`INCR` + `EXPIRE` nếu == 1); `check_mixed_limit`
      gộp 2 keys vào 1 script → mixed limit 4 RTT còn 1, atomic, hết race rò rỉ key
      không TTL. Dùng `redis::Script` (tự fallback `EVAL` khi thiếu `EVALSHA`).
      Giữ nguyên fail-open khi Redis lỗi + test tương đương.
- [ ] `COUNT(*) OVER()` gộp COUNT + SELECT thành 1 query cho expense và settlement
      list (2 RTT còn 1). Không đổi signature repo. Kiểm chứng tổng bằng test hiện có.
- [ ] Cache membership `group:members:{id}` (danh sách `(user_id, role)`, TTL 60s):
      `ensure_membership`/`ensure_admin` và các read path dùng cache trước, miss mới
      query. Xóa exact key khi add/remove member và delete group. Ghi rõ trade-off:
      staleness tối đa 60s nếu sót điểm invalidate (hiện chưa có endpoint remove-member).
- [ ] Pipeline Redis cho các op cache đi theo batch (`sign_out`, `refresh`,
      `change_password`, `reset_password`: get + delete×N + set×2 → 1 batch).

Acceptance: đếm query/RTT giảm theo từng endpoint (ghi vào commit), fail-open giữ
nguyên (test), `cargo test` green.

## Phase 7 — Alloc, hotpath, query hygiene

Branch: `phase7-alloc-hotpath` (stack trên tip Phase 6).

- [ ] Mapper nhận by-value thay vì `&` + clone (mọi call site đều sở hữu entity,
      map ở tail) — hết clone từng field mỗi response.
- [ ] Gom shares theo expense bằng `HashMap` (O(E+S)) thay `filter` O(E×S)
      (`expenses/pg.rs`), đối chiếu bench `share_grouping` Phase 5.
- [ ] `hashbrown` (hasher foldhash) cho 2 map nóng: `balance_map` (debt engine),
      `user_name_map` (summary). Không đổi API (cùng interface `HashMap`).
- [ ] `SmallVec` cho vec nhỏ (shares, debtors/creditors) + rà `with_capacity`;
      `#[inline]` hàm nóng nhỏ (`validate_sum`, pagination helpers). Chỉ giữ mục nào
      bench cải thiện.
- [ ] `SELECT *` → liệt kê cột ở query nóng, trước hết `find_members` (chạy trên
      ~mọi request): chỉ lấy cột service/mapper thật sự dùng.

Acceptance: criterion alloc/op và time/op giảm (hoặc giữ nguyên thì revert mục đó),
clippy sạch, test xanh, response JSON không đổi field nào.

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
