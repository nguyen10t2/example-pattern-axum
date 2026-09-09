# SplitDebt API

Backend quản lý chi tiêu nhóm: tạo nhóm, ghi expense chia tiền, gợi ý trả nợ tối thiểu.
Viết bằng Rust (edition 2024), framework web Axum, PostgreSQL + Redis.

## Tính năng

- **Xác thực**: OTP qua email, JWT access/refresh token kèm xoay vòng (rotation) và giới hạn
  session, đăng nhập Google OAuth 2.0 (PKCE).
- **Nhóm**: tạo nhóm, tham gia bằng invite code, phân quyền admin/member.
- **Expense**: chia đều (equal), chia số tiền cụ thể (exact), chia theo phần trăm (percentage);
  validate phía server, lưu trong transaction.
- **Settlement**: ghi nhận trả nợ, engine gợi ý số giao dịch tối thiểu (greedy + exact-match).
- **Hạ tầng**: rate limiting và cache trên Redis (fail-open khi Redis lỗi), cache tóm tắt nhóm,
  response song ngữ Việt/Anh, security headers, CORS, graceful shutdown.

## Yêu cầu

- Rust stable mới nhất (khuyến nghị qua `rustup`).
- PostgreSQL 14+.
- Redis 6+.

## Cấu hình

Copy `.env` từ mẫu sau (file `.env` không commit lên git):

```bash
DATABASE_URL=postgresql://user:password@localhost:5432/splitdebt
REDIS_URL=redis://127.0.0.1:6379

HOST=0.0.0.0
PORT=3000
FRONTEND_URL=http://localhost:5173
RUST_LOG=dsa=debug,tower_http=debug,axum::rejection=trace

JWT_SECRET=doi-secret-it-nhat-32-ky-tu-tai-day
JWT_ISSUER=splitdebt
JWT_AUDIENCE=splitdebt-users

GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=
GOOGLE_REDIRECT_URI=http://localhost:5173/api/users/auth/google/callback
```

Các giá trị tinh chỉnh thêm (đều có mặc định, xem `src/config/constants.rs`):

| Biến | Mặc định | Ý nghĩa |
|---|---|---|
| `DB_MAX_CONNECTIONS` | `5` | Số connection tối đa trong pool |
| `DB_MIN_CONNECTIONS` | `1` | Số connection idle tối thiểu |
| `DB_ACQUIRE_TIMEOUT` | `10` (giây) | Chờ connection tối đa, quá thì fail nhanh |
| `REDIS_CONNECTION_TIMEOUT` | `5` (giây) | Timeout bắt tay Redis + chặn trên lúc khởi động |
| `REDIS_RESPONSE_TIMEOUT` | `2` (giây) | Timeout mỗi lệnh Redis |
| `ARGON2_M_COST` / `ARGON2_T_COST` / `ARGON2_P_COST` | `19456` / `2` / `1` | Tham số băm mật khẩu Argon2id |

## Chạy

```bash
cargo build
sqlx migrate run
cargo run
```

Server lắng nghe tại `http://HOST:PORT` (mặc định `http://0.0.0.0:3000`).
Khởi động fail nhanh (`exit 1` kèm log) nếu cấu hình sai, Redis unreachable hoặc migrate lỗi.

## Kiểm thử và chất lượng

```bash
cargo test                # unit + integration + system
cargo test -- --nocapture # hiện stdout khi debug test
cargo fmt --check         # kiểm tra format (rustfmt.toml)
cargo clippy --all-targets # lint ở mức pedantic + nursery (đã khóa trong Cargo.toml)
```

Bố cục test: `tests/unit` (logic thuần), `tests/integration` (service + API),
`tests/system` (fail-fast khi Redis lỗi). Hai case cần Postgres thật đang `#[ignore]`
kèm lý do, chạy riêng khi có `TEST_DATABASE_URL`.

## API

Base path `/api`. Xác thực bằng Bearer access token, trừ các route public.

| Nhóm | Route chính |
|---|---|
| Users | `POST /api/users/request-otp`, `POST /api/users/signup`, `POST /api/users/signin`, `POST /api/users/refresh`, `POST /api/users/signout`, `GET/PATCH /api/users/me`, `POST /api/users/me/password`, `GET /api/users/auth/google`, `GET /api/users/auth/google/callback` |
| Groups | `GET/POST /api/groups`, `POST /api/groups/join`, `GET/DELETE /api/groups/{id}`, `GET /api/groups/{id}/summary`, `GET/POST /api/groups/{id}/members` |
| Expenses | `POST /api/expenses`, `GET/DELETE /api/expenses/{id}`, `GET /api/expenses/group/{id}` (phân trang) |
| Settlements | `POST /api/settlements`, `GET/DELETE /api/settlements/{id}`, `GET /api/settlements/group/{id}` (phân trang) |

Định dạng response thống nhất: `{ "success": true, "message": "...", "data": ... }`;
lỗi trả `{ "success": false, "message": "..." }` kèm HTTP status và mã lỗi.

## Cấu trúc mã nguồn

```
src/
├── main.rs        # Điểm vào: tracing, routes, middleware, graceful shutdown
├── state.rs       # AppState (composition root, fail-fast)
├── config/        # Builder cấu hình + constants dùng chung
├── domain/        # Logic nghiệp vụ theo domain: users, groups, expenses, settlements
│   └── <domain>/  # entity, repository (trait), pg (Postgres impl), service,
│                  # handle (HTTP), request, response, mapper
├── errors/        # Phân loại lỗi: AppError, BusinessError, SystemError
├── middleware/    # Auth JWT, rate limiter Redis, validate, security headers
├── responses/     # Envelope response thành công
└── utils/         # Cache, JWT, OAuth, mailer, hash Argon2, i18n
migrations/        # Migration SQLx
tests/             # unit, integration, system
```

Tầng service phụ thuộc vào repository trait (không phụ thuộc Postgres trực tiếp) nên
test được bằng mock/fake; cache có 2 backend (`Redis` production, `Memory` cho test).

## Giấy phép

MIT — xem file `LICENSE`.
