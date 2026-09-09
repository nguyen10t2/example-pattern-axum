use argon2::{Algorithm, Argon2, Params, Version};

pub mod constants;

// ---------------------------------------------------------------------------
// Argon2Config + Builder
// ---------------------------------------------------------------------------

/// Configuration for Argon2 password hashing.
#[derive(Debug, Clone)]
pub struct Argon2Config {
    pub memory_cost: u32,
    pub time_cost: u32,
    pub parallelism: u32,
    pub output_len: usize,
}

/// Builder for [`Argon2Config`].
///
/// All fields default to the recommended Argon2id parameters so you can
/// call `Argon2ConfigBuilder::new().build()` and get a sensible config.
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
    /// Creates a new builder with default Argon2id parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the memory cost (in KiB). Default: `Params::DEFAULT_M_COST` (19456 KiB ≈ 19 MiB).
    #[must_use]
    pub fn memory_cost(mut self, cost: u32) -> Self {
        self.memory_cost = cost;
        self
    }

    /// Sets the time cost (number of iterations). Default: `Params::DEFAULT_T_COST` (2).
    #[must_use]
    pub fn time_cost(mut self, cost: u32) -> Self {
        self.time_cost = cost;
        self
    }

    /// Sets the parallelism (number of threads). Default: `Params::DEFAULT_P_COST` (1).
    #[must_use]
    pub fn parallelism(mut self, p: u32) -> Self {
        self.parallelism = p;
        self
    }

    /// Sets the output length in bytes. Default: 32.
    #[must_use]
    pub fn output_len(mut self, len: usize) -> Self {
        self.output_len = len;
        self
    }

    /// Populates fields from environment variables, falling back to the
    /// current builder values (which are the defaults unless overridden).
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

    /// Consumes the builder and returns an [`Argon2Config`].
    #[must_use]
    pub fn build(self) -> Argon2Config {
        Argon2Config {
            memory_cost: self.memory_cost,
            time_cost: self.time_cost,
            parallelism: self.parallelism,
            output_len: self.output_len,
        }
    }
}

impl Argon2Config {
    /// Returns a builder pre-populated from environment variables.
    #[must_use]
    pub fn builder() -> Argon2ConfigBuilder {
        Argon2ConfigBuilder::new()
    }

    /// Convenience shortcut: `Argon2ConfigBuilder::new().from_env().build()`.
    #[must_use]
    pub fn from_env() -> Self {
        Self::builder().from_env().build()
    }

    /// Builds the `Argon2` hasher from this configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the parameters are invalid for Argon2.
    pub fn build_argon2(&self) -> Result<Argon2<'static>, argon2::Error> {
        let params = Params::new(self.memory_cost, self.time_cost, self.parallelism, Some(self.output_len))?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

// ---------------------------------------------------------------------------
// DatabaseConfig + Builder
// ---------------------------------------------------------------------------

/// Configuration for the `PostgreSQL` connection pool.
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_slow_threshold: std::time::Duration,
    pub acquire_timeout: std::time::Duration,
}

/// Builder for [`DatabaseConfig`].
///
/// # Required
///
/// - `database_url` — must be set via [`DatabaseConfigBuilder::database_url`]
///   or [`DatabaseConfigBuilder::from_env`] before calling [`build`](DatabaseConfigBuilder::build).
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
    /// Creates a new builder with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the PostgreSQL connection URL. **Required.**
    #[must_use]
    pub fn database_url(mut self, url: impl Into<String>) -> Self {
        self.database_url = Some(url.into());
        self
    }

    /// Sets the maximum number of connections in the pool. Default: 5.
    #[must_use]
    pub fn max_connections(mut self, n: u32) -> Self {
        self.max_connections = n;
        self
    }

    /// Sets the minimum number of idle connections. Default: 1.
    #[must_use]
    pub fn min_connections(mut self, n: u32) -> Self {
        self.min_connections = n;
        self
    }

    /// Sets the slow-acquire warning threshold. Default: 2 seconds.
    #[must_use]
    pub fn acquire_slow_threshold(mut self, d: std::time::Duration) -> Self {
        self.acquire_slow_threshold = d;
        self
    }

    /// Sets the maximum time [`sqlx::Pool::acquire`] will wait for a connection
    /// before returning an error. This also bounds how long the pool waits while
    /// trying to open a new connection to an unreachable database, so the server
    /// fails fast instead of hanging forever. Default: 10 seconds.
    #[must_use]
    pub fn acquire_timeout(mut self, d: std::time::Duration) -> Self {
        self.acquire_timeout = d;
        self
    }

    /// Populates fields from environment variables, falling back to the
    /// current builder values.
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

    /// Consumes the builder and returns a [`DatabaseConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::MissingDatabaseUrl`] if `database_url` was never set.
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
    /// Returns a new builder.
    #[must_use]
    pub fn builder() -> DatabaseConfigBuilder {
        DatabaseConfigBuilder::new()
    }

    /// Convenience shortcut: reads from env and panics if `DATABASE_URL` is missing.
    ///
    /// # Panics
    ///
    /// Panics if the `DATABASE_URL` environment variable is not set.
    #[must_use]
    pub fn from_env() -> Self {
        Self::builder().from_env().build().expect("DATABASE_URL must be set")
    }

    /// Creates a new database pool with the configured settings.
    ///
    /// # Errors
    ///
    /// Returns an error if the database connection cannot be established.
    ///
    /// Note: the pool is created lazily. `acquire_timeout` bounds how long the
    /// pool waits while opening a connection, so an unreachable database surfaces
    /// as an error instead of hanging the process.
    pub fn connect_lazy(&self) -> Result<sqlx::PgPool, sqlx::Error> {
        let options = self.database_url.parse::<sqlx::postgres::PgConnectOptions>()?;

        Ok(sqlx::postgres::PgPoolOptions::new()
            .max_connections(self.max_connections)
            .min_connections(self.min_connections)
            .acquire_slow_threshold(self.acquire_slow_threshold)
            .acquire_timeout(self.acquire_timeout)
            .connect_lazy_with(options))
    }

    /// Runs database migrations using the configured database pool.
    ///
    /// # Errors
    ///
    /// Returns an error if migrations fail to run.
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

/// Configuration for the `Redis` connection manager.
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub connection_timeout: std::time::Duration,
    pub response_timeout: std::time::Duration,
}

/// Builder for [`RedisConfig`].
///
/// # Defaults
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
    /// Creates a new builder with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Redis connection URL. Default: `redis://127.0.0.1:6379`.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the per-attempt timeout for establishing a Redis connection.
    /// This is also used as the overall bound for the initial connect so the
    /// server fails fast instead of hanging forever. Default: 5 seconds.
    #[must_use]
    pub fn connection_timeout(mut self, d: std::time::Duration) -> Self {
        self.connection_timeout = d;
        self
    }

    /// Sets the timeout for each Redis command (applied via
    /// `ConnectionManagerConfig::set_response_timeout`), so a hung Redis
    /// surfaces as an error instead of hanging request handlers. Default: 2 seconds.
    #[must_use]
    pub fn response_timeout(mut self, d: std::time::Duration) -> Self {
        self.response_timeout = d;
        self
    }

    /// Populates fields from environment variables, falling back to the
    /// current builder values.
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

    /// Consumes the builder and returns a [`RedisConfig`].
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
    /// Returns a new builder.
    #[must_use]
    pub fn builder() -> RedisConfigBuilder {
        RedisConfigBuilder::new()
    }

    /// Convenience shortcut: `RedisConfigBuilder::new().from_env().build()`.
    #[must_use]
    pub fn from_env() -> Self {
        Self::builder().from_env().build()
    }

    /// Establishes a Redis connection manager with timeouts applied.
    ///
    /// The per-attempt TCP timeout and the per-command response timeout are
    /// taken from this config, and the total initial-connect wait is capped at
    /// `connection_timeout` so an unreachable Redis fails fast instead of
    /// hanging startup forever (retries are kept low on purpose).
    ///
    /// # Errors
    ///
    /// Returns a [`RedisConnectError`] if the URL is invalid, the initial
    /// connect times out, or the connection cannot be established.
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

/// Errors that can occur while establishing a Redis connection via [`RedisConfig::connect`].
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
