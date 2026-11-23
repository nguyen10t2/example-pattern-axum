# Repository Guidelines

## Project Structure & Module Organization

```
src/
├── main.rs              # Application entry point
├── lib.rs               # Library root
├── state.rs             # Application state definitions
├── config/mod.rs        # Configuration management
├── domain/              # Business logic (domain-driven design)
│   └── users/           # User domain: entity, repository, service, handlers
├── errors/              # Error types (app, business, system, sqlx)
├── middleware/           # HTTP middleware (error logging)
├── responses/           # Response types (success, etc.)
└── utils/               # Utilities (hashing, etc.)
migrations/              # SQLx database migrations
Cargo.toml               # Project manifest and dependencies
rustfmt.toml             # Rust formatting configuration
```

## Build, Test, and Development Commands

```bash
cargo build               # Compile the project
cargo build --release     # Compile with optimizations
cargo test                # Run all tests
cargo test -- --nocapture # Run tests with stdout visible
cargo fmt                 # Format code using rustfmt.toml
cargo fmt --check         # Check formatting without modifying files
cargo clippy              # Run linter for common mistakes
sqlx migrate run          # Apply pending database migrations
```

## Coding Style & Naming Conventions

- **Indentation**: 4 spaces (configured in `rustfmt.toml`)
- **Max line width**: 120 characters
- **Naming**: `snake_case` for functions/variables, `PascalCase` for types/traits
- **Formatting**: Use `cargo fmt` with `rustfmt.toml` before committing
- **Async runtime**: Tokio (full features enabled)

## Testing Guidelines

- **Framework**: Built-in `#[test]` and `#[tokio::test]` for async tests
- **Location**: Tests live in `#[cfg(test)]` modules within source files
- **Naming**: Descriptive names like `test_hash_success`, `test_verify_success`
- **Run all tests**: `cargo test`
- **Run specific test**: `cargo test test_name`

## Commit & Pull Request Guidelines

- **Commits**: Use imperative mood (e.g., "Add user repository", "Fix hash verification")
- **Structure**: Keep commits focused on single logical changes
- **PRs**: Include description of changes and link to related issues
- **Formatting**: Run `cargo fmt` and `cargo clippy` before submitting

## Security & Configuration

- **Environment**: Uses `.env` file (gitignored) for secrets
- **Database**: PostgreSQL with SQLx (parameterized queries prevent SQL injection)
- **Passwords**: Argon2id hashing via `argon2` crate

## Quy Tắc Bắt Buộc

### Code Quality

- **KHÔNG** hardcode magic values — dùng constants hoặc config
- **KHÔNG** code smell — mỗi function 1 trách nhiệm, tập trung. Nếu vượt ~60–80 dòng, cần xem xét tách nhỏ hoặc giải thích lý do
- **KHÔNG** vẽ lại bánh xe — dùng crates đã có (xem workspace.dependencies)
- **Mọi** public function phải có doc comments (`///`)
- **Mọi** error type dùng `thiserror`
- **Mọi** async function phải justify tại sao async
- **Mọi** cargo clippy warnings phải fix thật không dùng các cờ đề pass qua production code

### Error Handling

- **Production code:** Không `unwrap()`, không `expect()` — propagate errors bằng `?`
- **Tests:** `unwrap()`/`expect()` được phép khi giúp test rõ ràng hơn (Tokio, Hyper, Quinn, rustls cũng làm vậy)
- Không `panic!()` trong production code — `panic!()` chỉ trong tests hoặc unreachable invariants
- Mọi error phải preserve context
- Library **KHÔNG** dùng `anyhow` — chỉ trả về typed errors (`thiserror`)
- Binary/app mới được dùng `anyhow`

### Async Safety

- KHÔNG giữ lock qua `.await`
- KHÔNG blocking trong async runtime
- KHÔNG `std::thread::sleep`
- KHÔNG `std::sync::Mutex` trong async code trừ khi chứng minh được
- KHÔNG `std::sync::RwLock` trong async code trừ khi chứng minh được (giống Mutex)
- Mọi blocking I/O phải `spawn_blocking`
- Async function phải cancellation-safe
- Không được bỏ quên JoinHandle

### Performance

- Ưu tiên borrowing thay vì clone
- Clone phải có lý do
- Ưu tiên `Bytes` thay vì `Vec<u8>` cho networking
- Dùng `SmallVec` khi collection thường nhỏ
- Zero-copy khi có thể
- Không allocate trong hot path nếu tránh được
- Benchmark trước khi optimize

### Concurrency Rules

- KHÔNG global locks
- KHÔNG `Arc<Mutex<T>>` unless justified
- Dùng actor model + message passing
- `spawn_blocking` cho mọi blocking I/O
- `CancellationToken` cho graceful shutdown
- `JoinSet` cho dynamic tasks

### Tokio Patterns

Ưu tiên: `watch`, `Notify`, `Semaphore`, `CancellationToken`, `JoinSet`
Hạn chế: `Arc<Mutex<_>>`
Không spawn nếu chỉ cần await.
Không spawn trong loop nếu không có backpressure.

### Async Patterns (BẮT BUỘC)

- **Batch I/O:** Gom nhiều independent I/O thành 1 batch, dùng `tokio::join!` hoặc `tokio::try_join!`
  ```rust
  // ✅ ĐÚNG: parallel
  let (users, messages, groups) = tokio::try_join!(
      db.get_users(),
      db.get_messages(),
      db.get_groups(),
  )?;

  // ❌ SAI: sequential (3 round trips thay vì 1)
  let users = db.get_users().await?;
  let messages = db.get_messages().await?;
  let groups = db.get_groups().await?;
  ```
- **DB patterns:**
  - Dùng `ON CONFLICT DO NOTHING/UPDATE` thay vì find-then-insert (1 round trip thay vì 2)
  - Dùng `INSERT ... RETURNING` thay vì insert-then-select
  - Dùng batch insert (`INSERT INTO ... VALUES (...), (...), (...)`) thay vì loop insert
  - Dùng transactions cho multi-step operations
- **`tokio::select!`:** Dùng cho racing/cancellation, KHÔNG dùng cho sequential logic
- **`tokio::join!`:** Dùng cho independent parallel tasks
- **`tokio::try_join!`:** Dùng cho independent parallel tasks có thể fail
- **`JoinSet`:** Dùng cho dynamic number of tasks (file chunks, fan-out)
- **Channel patterns:**
  - `mpsc` cho producer-consumer (bounded = backpressure)
  - `broadcast` cho fan-out (1 sender, N receivers)
  - `watch` cho shared state (latest value wins)
  - `oneshot` cho one-shot response

### State Machines

Không dùng nhiều bool để biểu diễn state.

```rust
// ❌ SAI
connected: bool
authenticated: bool

// ✅ ĐÚNG
enum ConnectionState {
    Connecting,
    Authenticating,
    Ready,
    Closing,
}
```

### Protocol Design

Mỗi protocol phải có:

- Version
- Packet ID
- Message Type
- Serialization
- Error Codes
- Backward Compatibility

### Serialization

- Không serialize trực tiếp business objects — dùng DTO riêng
- Packet phải versioned
- Không dùng JSON cho hot path nếu binary phù hợp

### Observability

- `tracing` cho mọi async boundary
- Không `println!`
- Metrics cho: latency, throughput, queue length, dropped packets, reconnect count

### Memory

- Không tạo Arc nếu ownership đơn giản
- Không Rc trong async
- Không leak JoinHandle
- Không leak channels
- Không `mem::forget` — dùng proper drop semantics (store permits in HashMap, drop on remove)
- `Bytes` clone là O(1) (Arc-backed) — không cần wrap trong Arc thêm

### API Design

- Tránh bool parameters
- Strong types
- Builder Pattern cho config lớn
- Typestate khi phù hợp

### Security

- Không log plaintext
- Không log secret
- Không log private key
- Zeroize secrets khi có thể
- Crypto và networking tách riêng

### Review Checklist

Trước khi kết thúc bất kỳ task nào, luôn trả lời:

- Có race condition không?
- Có deadlock không?
- Có giữ lock qua await không?
- Có task leak không?
- Có cancellation-safe không?
- Có unnecessary clone không?
- Có unnecessary Arc không?
- Có allocation trong hot path không?
- Có benchmark không?
- Có test edge cases chưa?
- Clippy sạch chưa?
- Docs đầy đủ chưa?

### Testing (CỰC KỲ QUAN TRỌNG)

- Mỗi crate có `tests/` folder với:
  - `tests/unit/` — unit tests
  - `tests/integration/` — integration tests
  - `tests/system/` — system tests
- Độ bao phủ test phải CAO — test mọi edge case
- Dùng crates test xịn:
  - `tokio::test` cho async tests
  - `proptest` cho property-based testing
  - `mockall` cho mocking
  - `insta` cho snapshot testing
  - `loom` cho concurrency testing
  - `cargo-fuzz` cho fuzz testing
- Test naming: `test_<function>_<scenario>_<expected>`

### Benchmarking

- KHÔNG dùng `std::time::Instant` cho benchmark
- Dùng `criterion` cho benchmarks
- Dùng `iai-callgrind` cho instruction-level benchmarks
- Benchmarks trong `benches/` folder
- Benchmark phải report: Throughput, Latency p50/p95/p99, Allocations, Instruction count, Memory usage

### Clippy

- Clippy ở mức MẠNH: `clippy::all`, `clippy::pedantic`, `clippy::nursery`
- Không cho phép clippy warnings

### Git Workflow

- Mỗi agent làm việc trên git worktree riêng (nếu cần)
- KHÔNG bao giờ code cùng một file
- KHÔNG bao giờ code cùng một tính năng
- Merge sau khi test pass + review xong

### Process Workflow (Hybrid: milestone-gate + sprint)

Áp dụng từ **Phase 7** trở đi. Tham chiếu thực thi đầu tiên:
[docs/PLAN_PHASE7_MOBILE_PUSH.md](docs/PLAN_PHASE7_MOBILE_PUSH.md).

| Cấp | Quy tắc |
|---|---|
| **Milestone (waterfall gate)** | Có Entry + Exit criteria viết trong PLAN doc. Đóng gate bắt buộc: tests xanh toàn workspace, clippy 0 warning, tick exit criteria kèm evidence, viết `DONE_<PHASE>_<M#>.md`, merge nhánh về `feat/nguyen`. Gate trước chưa đóng thì không mở gate sau. |
| **Sprint (agile nhẹ)** | Nhóm 3–6 task trong 1 milestone; mỗi task 0.5–2 ngày có Acceptance riêng; đo theo task hoàn thành, KHÔNG cam kết lịch ngày cứng. Re-plan đầu mỗi milestone. |
| **Nhánh** | `feat/<phase>-m<n>-<ten-ngan>` từ `feat/nguyen` (vd `feat/phase7a-m1-server-wiring`). Docs: `docs/<ten-ngan>`. Conventional Commits bắt buộc. |

#### Template DONE (bắt buộc khi đóng gate)

```markdown
# DONE <PHASE> — Milestone <M#>: <Tên>

## Kết quả
- <điểm chính, 1 dòng mỗi mục>

## Evidence
- Tests: <tên test / lệnh chạy, số case>
- Clippy: 0 warning (lệnh + ngày)
- Thủ công: <screenshot/log nếu có>

## Lệch kế hoạch
- <task bị bỏ/dời và lý do, hoặc "không">

## Follow-up cho milestone sau
- <mục hoặc "không">
```

### Naming Conventions

- Types: `PascalCase`
- Functions/methods: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Modules: `snake_case`
- Error types: `<Name>Error` (e.g., `ProtocolError`, `CryptoError`)

## Optimization Rules (POC2)

### Memory

- **`Bytes` thay `Vec<u8>`** cho networking, parsing, chia sẻ buffer
- **`BytesMut`** cho write buffers — dùng `freeze()` để chuyển thành `Bytes` (zero-cost)
- **`split_to()`** để tách buffer mà không copy (O(1))
- **`SmallVec<[T; N]>`** khi collection thường nhỏ (≤16) nhưng thỉnh thoảng lớn hơn
- **`Vec::with_capacity()`** khi biết trước kích thước
- **Reuse buffers** — truyền buffer vào function thay vì tạo mới mỗi lần
- **`bumpalo`** cho parser/AST — arena allocation, giải phóng hàng loạt

### Concurrency

- **`DashMap`** cho concurrent HashMap (sharded RwLock)
- **`scc`** nếu cần performance cao hơn DashMap
- **`arc-swap`** cho config hot-reload (read-heavy)
- **Bounded channels** mặc định — backpressure tránh OOM
- **`watch::channel()`** cho broadcast giá trị mới nhất
- **`broadcast::channel()`** cho fan-out messages

### Async

- **`tokio::task::yield_now()`** khi thực hiện công việc dài trong async
- **`spawn_blocking()`** cho mọi blocking I/O (SQLite, file, DNS)
- **`JoinSet`** cho dynamic tasks (không collect JoinHandles)
- **`CancellationToken`** cho graceful shutdown
- **Runtime config:** `worker_threads = num_cpus`, `max_blocking_threads = 512`

### QUIC

- **Stream priorities:** CHƯA được enforce — quinn 0.11 không expose per-stream priority API; thứ tự xử lý thực tế do thứ tự mở stream và dispatch quyết định
- **Datagram:** chưa dùng trong codebase — mọi traffic đi qua bidirectional streams
- **`initial_window`:** chưa cấu hình — đang dùng default của quinn

> Các mục dưới đây là hướng dẫn cho tương lai, CHƯA có trong code:
>
> - **`send_datagram_wait()`** để prioritize old datagrams
> - **`send_fairness(true)`** cho round-robin same priority
> - **`initial_window = 1MB`** cho throughput tốt hơn

### Crypto

- **ChaCha20-Poly1305 cho TOÀN BỘ stack** (E2E, at-rest, file, channel key) — nhất quán mọi platform, không thêm dependency AES-GCM (quyết định chốt `PLAN_REFACTOR` §1)
- **Parallel encryption** với `rayon` khi encrypt nhiều chunks

### Database

- **Connection pool:** `max_connections=20`, `min_connections=5`
- **Batch insert** với `sqlx::QueryBuilder`
- **`ON CONFLICT`** cho upsert (1 round trip thay vì 2)
- **Prepared statements** — `sqlx::query_as()` tự động prepare

### Compiler

- **Release profile:** `opt-level=3`, `lto="fat"`, `codegen-units=1`, `panic="abort"`
- **`target-cpu=native`** cho build local
- **PGO** (+10-30% runtime) — `cargo-pgo`
- **Faster linkers:** mold > lld > default

### Profiling

- **`criterion`** cho micro-benchmarks (KHÔNG dùng std::time)
- **`cargo-flamegraph`** cho flamegraph
- **`tokio-console`** cho async diagnostics
- **`tracing`** cho application-level spans
