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
        cache::{CacheStore, RedisCache},
        email::Mailer,
        jwt::JwtConfig,
        oauth::GoogleOAuthConfig,
    },
};

/// Errors that can occur while building [`AppState`] from the environment.
///
/// All variants are fatal at startup: the caller is expected to log the error
/// and exit instead of serving traffic with a half-initialized state.
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
    pub cache: Arc<dyn CacheStore>,
    pub rate_limiter: RedisRateLimiter,
    pub jwt_config: JwtConfig,
    pub google_oauth: GoogleOAuthConfig,
    pub argon2: Arc<Argon2<'static>>,
    pub db_pool: sqlx::PgPool,
    pub db_config: Arc<DatabaseConfig>,
}

impl AppState {
    /// Builds application state from the environment.
    ///
    /// # Errors
    ///
    /// Returns an [`AppStateError`] if any required configuration is invalid
    /// or if Redis cannot be reached within `REDIS_CONNECTION_TIMEOUT`.
    /// The caller must treat this as fatal and exit (fail-fast) instead of
    /// serving traffic with a half-initialized state.
    pub async fn from_env() -> Result<Self, AppStateError> {
        let argon2 = Argon2Config::from_env().build_argon2()?;
        let argon2_arc = Arc::new(argon2);
        let db_config = DatabaseConfig::builder().from_env().build()?;
        let pool = db_config.connect_lazy()?;
        tracing::debug!("database pool created (lazy; first connection deferred until first query)");

        let redis_config = RedisConfig::from_env();
        let connection_manager = redis_config.connect().await?;

        let cache: Arc<dyn CacheStore> = Arc::new(RedisCache::new(connection_manager.clone()));
        let rate_limiter = RedisRateLimiter::new(connection_manager.clone());
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
            user_repo.clone(),
            cache.clone(),
            pool.clone(),
        ));

        let expense_service =
            Arc::new(ExpenseService::new(expense_repo.clone(), group_repo.clone(), cache.clone(), pool.clone()));

        let settlement_service =
            Arc::new(SettlementService::new(settlement_repo.clone(), group_repo.clone(), cache.clone(), pool.clone()));

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

    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        self.db_config.migrate(&self.db_pool).await
    }
}
