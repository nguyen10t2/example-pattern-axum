use argon2::Argon2;
use std::sync::Arc;

use crate::{
    config::{Argon2Config, DatabaseConfig, RedisConfig, constants::MAILER_BUFFER_SIZE},
    domain::{
        expenses::{pg::PostgresExpenseRepository, service::ExpenseService},
        groups::{pg::PostgresGroupRepository, service::GroupService},
        settlements::{pg::PostgresSettlementRepository, service::SettlementService},
        users::{pg::PostgresUserRepository, service::UserService},
    },
    middleware::RedisRateLimiter,
    utils::{
        cache::{Cache, RedisCache},
        email::Mailer,
        jwt::JwtConfig,
        oauth::GoogleOAuthConfig,
    },
};

/// Các lỗi dựng [`AppState`] — đều fatal lúc boot: caller log rồi exit.
#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    #[error("invalid argon2 config: {0}")]
    InvalidArgon2(#[from] argon2::Error),
    #[error("invalid database configuration: {0}")]
    InvalidDatabase(#[from] crate::config::ConfigError),
    #[error("invalid database url: {0}")]
    InvalidDatabaseUrl(#[from] sqlx::Error),
    #[error(transparent)]
    Redis(#[from] crate::config::RedisConnectError),
}

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService<PostgresUserRepository>>,
    pub group_service: Arc<
        GroupService<
            PostgresGroupRepository,
            PostgresExpenseRepository,
            PostgresSettlementRepository,
            PostgresUserRepository,
        >,
    >,
    pub expense_service: Arc<ExpenseService<PostgresExpenseRepository, PostgresGroupRepository>>,
    pub settlement_service: Arc<SettlementService<PostgresSettlementRepository, PostgresGroupRepository>>,
    pub cache: Arc<Cache>,
    pub rate_limiter: RedisRateLimiter,
    pub jwt_config: JwtConfig,
    pub google_oauth: GoogleOAuthConfig,
    pub argon2: Arc<Argon2<'static>>,
    pub db_pool: sqlx::PgPool,
    pub db_config: Arc<DatabaseConfig>,
}

impl AppState {
    /// Dựng state từ env.
    ///
    /// # Errors
    ///
    /// Trả `AppStateError` khi config sai hoặc Redis unreachable — caller phải exit (fail-fast).
    pub async fn from_env() -> Result<Self, AppStateError> {
        let argon2 = Argon2Config::from_env().build_argon2()?;
        let argon2_arc = Arc::new(argon2);
        let db_config = DatabaseConfig::from_env()?;
        let pool = db_config.connect_lazy()?;
        tracing::debug!("database pool created (lazy; first connection deferred until first query)");

        let redis_config = RedisConfig::from_env();
        let connection_manager = redis_config.connect().await?;

        let cache = Arc::new(Cache::Redis(RedisCache::new(connection_manager.clone())));
        let rate_limiter = RedisRateLimiter::new(connection_manager);
        let jwt_config = JwtConfig::from_env();
        let google_oauth = GoogleOAuthConfig::from_env();

        let user_repo = PostgresUserRepository::new();
        let expense_repo = PostgresExpenseRepository::new();
        let group_repo = PostgresGroupRepository::new();
        let settlement_repo = PostgresSettlementRepository::new();

        let mailer = Mailer::new(MAILER_BUFFER_SIZE);

        let user_service = Arc::new(UserService::new(
            user_repo.clone(),
            cache.clone(),
            argon2_arc.clone(),
            jwt_config.clone(),
            mailer,
            pool.clone(),
        ));

        let group_service = Arc::new(GroupService::new(
            group_repo.clone(),
            expense_repo.clone(),
            settlement_repo.clone(),
            user_repo,
            cache.clone(),
            pool.clone(),
        ));

        let expense_service =
            Arc::new(ExpenseService::new(expense_repo, group_repo.clone(), cache.clone(), pool.clone()));

        let settlement_service =
            Arc::new(SettlementService::new(settlement_repo, group_repo, cache.clone(), pool.clone()));

        Ok(Self {
            user_service,
            group_service,
            expense_service,
            settlement_service,
            cache,
            rate_limiter,
            jwt_config,
            google_oauth,
            argon2: argon2_arc,
            db_pool: pool,
            db_config: Arc::new(db_config),
        })
    }

    /// Chạy migrations pending trên pool đã khởi tạo.
    ///
    /// # Errors
    ///
    /// Trả `MigrateError` khi migration lỗi — caller phải exit, không được serve.
    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        self.db_config.migrate(&self.db_pool).await
    }
}
