use argon2::{Algorithm, Argon2, Params, Version};

pub mod constants;

// ---------------------------------------------------------------------------
// Argon2Config + Builder
// ---------------------------------------------------------------------------

/// Cấu hình băm password Argon2.
#[derive(Debug, Clone)]
pub struct Argon2Config {
    pub memory_cost: u32,
    pub time_cost: u32,
    pub parallelism: u32,
    pub output_len: usize,
}

/// Builder cho [`Argon2Config`], mặc định là tham số Argon2id khuyến nghị.
#[derive(Debug, Clone)]
pub struct Argon2ConfigBuilder {
    memory_cost: u32,
    time_cost: u32,
    parallelism: u32,
    output_len: usize,
}

impl Default for Argon2ConfigBuilder {
    fn default() -> Self {
        Self {
            memory_cost: Params::DEFAULT_M_COST,
            time_cost: Params::DEFAULT_T_COST,
            parallelism: Params::DEFAULT_P_COST,
            output_len: 32,
        }
    }
}

impl Argon2ConfigBuilder {
    /// Tạo builder với tham số Argon2id mặc định.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Chi phí memory (KiB). Mặc định: ~19 MiB.
    #[must_use]
    pub const fn memory_cost(mut self, cost: u32) -> Self {
        self.memory_cost = cost;
        self
    }

    /// Số vòng lặp. Mặc định: 2.
    #[must_use]
    pub const fn time_cost(mut self, cost: u32) -> Self {
        self.time_cost = cost;
        self
    }

    /// Số luồng song song. Mặc định: 1.
    #[must_use]
    pub const fn parallelism(mut self, p: u32) -> Self {
        self.parallelism = p;
        self
    }

    /// Độ dài output (bytes). Mặc định: 32.
    #[must_use]
    pub const fn output_len(mut self, len: usize) -> Self {
        self.output_len = len;
        self
    }

    /// Nạp từ biến môi trường, fallback về giá trị hiện tại.
    ///
    /// # Environment Variables
    ///
    /// | Env var              | Field          |
    /// |----------------------|----------------|
    /// | `ARGON2_M_COST`      | memory_cost    |
    /// | `ARGON2_T_COST`      | time_cost      |
    /// | `ARGON2_P_COST`      | parallelism    |
    /// | `ARGON2_OUTPUT_LEN`  | output_len     |
    #[must_use]
    pub fn from_env(self) -> Self {
        Self {
            memory_cost: parse_env("ARGON2_M_COST", self.memory_cost),
            time_cost: parse_env("ARGON2_T_COST", self.time_cost),
            parallelism: parse_env("ARGON2_P_COST", self.parallelism),
            output_len: parse_env("ARGON2_OUTPUT_LEN", self.output_len),
        }
    }

    /// Build ra [`Argon2Config`].
    #[must_use]
    pub const fn build(self) -> Argon2Config {
        Argon2Config {
            memory_cost: self.memory_cost,
            time_cost: self.time_cost,
            parallelism: self.parallelism,
            output_len: self.output_len,
        }
    }
}

impl Argon2Config {
    /// Trả builder đã nạp từ env.
    #[must_use]
    pub fn builder() -> Argon2ConfigBuilder {
        Argon2ConfigBuilder::new()
    }

    /// Shortcut: `Argon2ConfigBuilder::new().from_env().build()`.
    #[must_use]
    pub fn from_env() -> Self {
        Self::builder().from_env().build()
    }

    /// Dựng hasher `Argon2` từ cấu hình.
    ///
    /// # Errors
    ///
    /// Trả lỗi khi tham số không hợp lệ với Argon2.
    pub fn build_argon2(&self) -> Result<Argon2<'static>, argon2::Error> {
        let params = Params::new(self.memory_cost, self.time_cost, self.parallelism, Some(self.output_len))?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

// ---------------------------------------------------------------------------
// DatabaseConfig + Builder
// ---------------------------------------------------------------------------

/// Cấu hình pool `PostgreSQL`.
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_slow_threshold: std::time::Duration,
    pub acquire_timeout: std::time::Duration,
}

/// Builder cho [`DatabaseConfig`]. Bắt buộc `database_url`; còn lại xem bảng mặc định.
///
/// # Defaults
///
/// | Field                   | Default |
/// |-------------------------|---------|
/// | max_connections         | 5       |
/// | min_connections         | 1       |
/// | acquire_slow_threshold  | 2 s     |
/// | acquire_timeout         | 10 s    |
#[derive(Debug, Clone)]
pub struct DatabaseConfigBuilder {
    database_url: Option<String>,
    max_connections: u32,
    min_connections: u32,
    acquire_slow_threshold: std::time::Duration,
    acquire_timeout: std::time::Duration,
}

impl Default for DatabaseConfigBuilder {
    fn default() -> Self {
        Self {
            database_url: None,
            max_connections: 5,
            min_connections: 1,
            acquire_slow_threshold: std::time::Duration::from_secs(2),
            acquire_timeout: std::time::Duration::from_secs(10),
        }
    }
}

impl DatabaseConfigBuilder {
    /// Tạo builder với giá trị mặc định.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// URL Postgres. **Bắt buộc.**
    #[must_use]
    pub fn database_url(mut self, url: impl Into<String>) -> Self {
        self.database_url = Some(url.into());
        self
    }

    /// Số connection tối đa. Mặc định: 5.
    #[must_use]
    pub const fn max_connections(mut self, n: u32) -> Self {
        self.max_connections = n;
        self
    }

    /// Số connection idle tối thiểu. Mặc định: 1.
    #[must_use]
    pub const fn min_connections(mut self, n: u32) -> Self {
        self.min_connections = n;
        self
    }

    /// Ngưỡng cảnh báo acquire chậm. Mặc định: 2 giây.
    #[must_use]
    pub const fn acquire_slow_threshold(mut self, d: std::time::Duration) -> Self {
        self.acquire_slow_threshold = d;
        self
    }

    /// Thời gian chờ `acquire` tối đa — DB unreachable thì fail nhanh thay vì treo.
    /// Mặc định: 10 giây.
    #[must_use]
    pub const fn acquire_timeout(mut self, d: std::time::Duration) -> Self {
        self.acquire_timeout = d;
        self
    }

    /// Nạp từ biến môi trường, fallback về giá trị hiện tại.
    ///
    /// # Environment Variables
    ///
    /// | Env var                   | Field                  |
    /// |---------------------------|------------------------|
    /// | `DATABASE_URL`            | database_url           |
    /// | `DB_MAX_CONNECTIONS`      | max_connections        |
    /// | `DB_MIN_CONNECTIONS`      | min_connections        |
    /// | `DB_ACQUIRE_SLOW_THRESHOLD` | acquire_slow_threshold (seconds) |
    /// | `DB_ACQUIRE_TIMEOUT`      | acquire_timeout (seconds) |
    #[must_use]
    pub fn from_env(self) -> Self {
        Self {
            database_url: Some(self.database_url.unwrap_or_else(|| std::env::var("DATABASE_URL").unwrap_or_default())),
            max_connections: parse_env("DB_MAX_CONNECTIONS", self.max_connections),
            min_connections: parse_env("DB_MIN_CONNECTIONS", self.min_connections),
            acquire_slow_threshold: std::time::Duration::from_secs(parse_env(
                "DB_ACQUIRE_SLOW_THRESHOLD",
                self.acquire_slow_threshold.as_secs(),
            )),
            acquire_timeout: std::time::Duration::from_secs(parse_env(
                "DB_ACQUIRE_TIMEOUT",
                self.acquire_timeout.as_secs(),
            )),
        }
    }

    /// Build ra [`DatabaseConfig`].
    ///
    /// # Errors
    ///
    /// Trả [`ConfigError::MissingDatabaseUrl`] khi chưa set `database_url`.
    pub fn build(self) -> Result<DatabaseConfig, ConfigError> {
        let database_url = self.database_url.ok_or(ConfigError::MissingDatabaseUrl)?;
        Ok(DatabaseConfig {
            database_url,
            max_connections: self.max_connections,
            min_connections: self.min_connections,
            acquire_slow_threshold: self.acquire_slow_threshold,
            acquire_timeout: self.acquire_timeout,
        })
    }
}

impl DatabaseConfig {
    /// Tạo builder mới.
    #[must_use]
    pub fn builder() -> DatabaseConfigBuilder {
        DatabaseConfigBuilder::new()
    }

    /// Đọc từ env.
    ///
    /// # Errors
    ///
    /// Trả [`ConfigError::MissingDatabaseUrl`] khi thiếu `DATABASE_URL`.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::builder().from_env().build()
    }

    /// Tạo pool lazy (kết nối thật ở query đầu). `acquire_timeout` chặn treo khi DB unreachable.
    ///
    /// # Errors
    ///
    /// Trả lỗi khi URL sai.
    pub fn connect_lazy(&self) -> Result<sqlx::PgPool, sqlx::Error> {
        let options = self.database_url.parse::<sqlx::postgres::PgConnectOptions>()?;

        Ok(sqlx::postgres::PgPoolOptions::new()
            .max_connections(self.max_connections)
            .min_connections(self.min_connections)
            .acquire_slow_threshold(self.acquire_slow_threshold)
            .acquire_timeout(self.acquire_timeout)
            .connect_lazy_with(options))
    }

    /// Chạy migrations trên pool đã khởi tạo.
    ///
    /// # Errors
    ///
    /// Trả lỗi khi migrate thất bại.
    pub async fn migrate(&self, pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
        tracing::debug!(
            "acquiring a database connection for migrations (acquire_timeout={}s)",
            self.acquire_timeout.as_secs()
        );
        tracing::info!("Running database migrations...");
        sqlx::migrate!("./migrations").run(pool).await?;
        tracing::info!("Database migrations completed successfully.");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// RedisConfig + Builder
// ---------------------------------------------------------------------------

/// Cấu hình connection manager `Redis`.
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub connection_timeout: std::time::Duration,
    pub response_timeout: std::time::Duration,
}

/// Builder cho [`RedisConfig`]. Mặc định:
///
/// | Field              | Default                   |
/// |--------------------|---------------------------|
/// | url                | `redis://127.0.0.1:6379`  |
/// | connection_timeout | 5 s                       |
/// | response_timeout   | 2 s                       |
#[derive(Debug, Clone)]
pub struct RedisConfigBuilder {
    url: Option<String>,
    connection_timeout: std::time::Duration,
    response_timeout: std::time::Duration,
}

impl Default for RedisConfigBuilder {
    fn default() -> Self {
        Self {
            url: None,
            connection_timeout: std::time::Duration::from_secs(5),
            response_timeout: std::time::Duration::from_secs(2),
        }
    }
}

impl RedisConfigBuilder {
    /// Tạo builder với giá trị mặc định.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// URL Redis. Mặc định: `redis://127.0.0.1:6379`.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Timeout mỗi lần bắt tay TCP, đồng thời là chặn trên cho lần connect đầu (fail nhanh).
    /// Mặc định: 5 giây.
    #[must_use]
    pub const fn connection_timeout(mut self, d: std::time::Duration) -> Self {
        self.connection_timeout = d;
        self
    }

    /// Timeout mỗi lệnh Redis (treo thì báo lỗi thay vì treo handler). Mặc định: 2 giây.
    #[must_use]
    pub const fn response_timeout(mut self, d: std::time::Duration) -> Self {
        self.response_timeout = d;
        self
    }

    /// Nạp từ biến môi trường, fallback về giá trị hiện tại.
    ///
    /// # Environment Variables
    ///
    /// | Env var                  | Field                  |
    /// |--------------------------|------------------------|
    /// | `REDIS_URL`              | url                    |
    /// | `REDIS_CONNECTION_TIMEOUT` | connection_timeout (seconds) |
    /// | `REDIS_RESPONSE_TIMEOUT`   | response_timeout (seconds)   |
    #[must_use]
    pub fn from_env(self) -> Self {
        Self {
            url: Some(self.url.unwrap_or_else(|| {
                std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
            })),
            connection_timeout: std::time::Duration::from_secs(parse_env(
                "REDIS_CONNECTION_TIMEOUT",
                self.connection_timeout.as_secs(),
            )),
            response_timeout: std::time::Duration::from_secs(parse_env(
                "REDIS_RESPONSE_TIMEOUT",
                self.response_timeout.as_secs(),
            )),
        }
    }

    /// Build ra [`RedisConfig`].
    #[must_use]
    pub fn build(self) -> RedisConfig {
        RedisConfig {
            url: self.url.unwrap_or_else(|| "redis://127.0.0.1:6379".to_string()),
            connection_timeout: self.connection_timeout,
            response_timeout: self.response_timeout,
        }
    }
}

impl RedisConfig {
    /// Tạo builder mới.
    #[must_use]
    pub fn builder() -> RedisConfigBuilder {
        RedisConfigBuilder::new()
    }

    /// Shortcut: `RedisConfigBuilder::new().from_env().build()`.
    #[must_use]
    pub fn from_env() -> Self {
        Self::builder().from_env().build()
    }

    /// Mở connection manager Redis có timeout (fail nhanh khi Redis unreachable).
    ///
    /// # Errors
    ///
    /// Trả `RedisConnectError` khi URL sai, timeout hoặc không kết nối được.
    #[tracing::instrument(skip(self), fields(
        connection_timeout_secs = self.connection_timeout.as_secs(),
        response_timeout_secs = self.response_timeout.as_secs(),
    ))]
    pub async fn connect(&self) -> Result<redis::aio::ConnectionManager, RedisConnectError> {
        tracing::debug!(
            "connecting to redis (connection_timeout={}s, response_timeout={}s)",
            self.connection_timeout.as_secs(),
            self.response_timeout.as_secs()
        );
        let client = redis::Client::open(self.url.as_str()).map_err(RedisConnectError::InvalidUrl)?;
        let manager_config = redis::aio::ConnectionManagerConfig::new()
            .set_connection_timeout(self.connection_timeout)
            .set_response_timeout(self.response_timeout)
            .set_number_of_retries(2);
        let manager = tokio::time::timeout(
            self.connection_timeout,
            redis::aio::ConnectionManager::new_with_config(client, manager_config),
        )
        .await
        .map_err(|_| RedisConnectError::ConnectionTimeout(self.connection_timeout.as_secs()))?
        .map_err(RedisConnectError::ConnectionFailed)?;
        tracing::debug!("redis connection established");
        Ok(manager)
    }
}

/// Lỗi mở kết nối Redis qua [`RedisConfig::connect`].
#[derive(Debug, thiserror::Error)]
pub enum RedisConnectError {
    #[error("invalid redis url: {0}")]
    InvalidUrl(#[source] redis::RedisError),
    #[error("timed out connecting to redis after {0}s")]
    ConnectionTimeout(u64),
    #[error("failed to connect to redis: {0}")]
    ConnectionFailed(#[source] redis::RedisError),
}

// ---------------------------------------------------------------------------
// ConfigError
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("DATABASE_URL is required but was not set")]
    MissingDatabaseUrl,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Argon2ConfigBuilder -------------------------------------------------

    #[test]
    fn test_argon2_builder_defaults() {
        let cfg = Argon2ConfigBuilder::new().build();
        assert_eq!(cfg.memory_cost, Params::DEFAULT_M_COST);
        assert_eq!(cfg.time_cost, Params::DEFAULT_T_COST);
        assert_eq!(cfg.parallelism, Params::DEFAULT_P_COST);
        assert_eq!(cfg.output_len, 32);
    }

    #[test]
    fn test_argon2_builder_overrides() {
        let cfg = Argon2ConfigBuilder::new().memory_cost(65536).time_cost(3).parallelism(4).output_len(64).build();
        assert_eq!(cfg.memory_cost, 65536);
        assert_eq!(cfg.time_cost, 3);
        assert_eq!(cfg.parallelism, 4);
        assert_eq!(cfg.output_len, 64);
    }

    #[test]
    fn test_argon2_build_argon2_success() {
        let cfg = Argon2ConfigBuilder::new().build();
        assert!(cfg.build_argon2().is_ok());
    }

    // -- DatabaseConfigBuilder -----------------------------------------------

    #[test]
    fn test_db_builder_missing_url() {
        let result = DatabaseConfigBuilder::new().build();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::MissingDatabaseUrl));
    }

    #[test]
    fn test_db_builder_with_url() {
        let cfg = DatabaseConfigBuilder::new()
            .database_url("postgresql://localhost/test")
            .max_connections(10)
            .min_connections(2)
            .build()
            .unwrap();
        assert_eq!(cfg.database_url, "postgresql://localhost/test");
        assert_eq!(cfg.max_connections, 10);
        assert_eq!(cfg.min_connections, 2);
    }

    #[test]
    fn test_db_builder_defaults_applied() {
        let cfg = DatabaseConfigBuilder::new().database_url("postgresql://localhost/test").build().unwrap();
        assert_eq!(cfg.max_connections, 5);
        assert_eq!(cfg.min_connections, 1);
        assert_eq!(cfg.acquire_slow_threshold, std::time::Duration::from_secs(2));
    }

    // -- RedisConfigBuilder --------------------------------------------------

    #[test]
    fn test_redis_builder_defaults_applied() {
        let cfg = RedisConfigBuilder::new().build();
        assert_eq!(cfg.url, "redis://127.0.0.1:6379");
        assert_eq!(cfg.connection_timeout, std::time::Duration::from_secs(5));
        assert_eq!(cfg.response_timeout, std::time::Duration::from_secs(2));
    }

    #[test]
    fn test_redis_builder_overrides() {
        let cfg = RedisConfigBuilder::new()
            .url("redis://localhost:6380")
            .connection_timeout(std::time::Duration::from_secs(1))
            .response_timeout(std::time::Duration::from_secs(1))
            .build();
        assert_eq!(cfg.url, "redis://localhost:6380");
        assert_eq!(cfg.connection_timeout, std::time::Duration::from_secs(1));
        assert_eq!(cfg.response_timeout, std::time::Duration::from_secs(1));
    }
}
