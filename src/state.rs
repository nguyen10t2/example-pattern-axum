use argon2::Argon2;
use std::sync::Arc;

use crate::{
    config::{Argon2Config, DatabaseConfig},
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
    pub async fn from_env() -> Self {
        let argon2 = Argon2Config::from_env().build_argon2().expect("invalid argon2 config");
        let argon2_arc = Arc::new(argon2);
        let db_config = DatabaseConfig::from_env();
        let pool = db_config.connect_lazy().expect("failed to connect to database");

        let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let redis_client = redis::Client::open(redis_url.as_str()).expect("invalid redis url");
        let connection_manager =
            redis::aio::ConnectionManager::new(redis_client).await.expect("failed to connect to redis");

        let cache: Arc<dyn CacheStore> = Arc::new(RedisCache::new(connection_manager.clone()));
        let rate_limiter = RedisRateLimiter::new(connection_manager.clone());
        let jwt_config = JwtConfig::from_env();
        let google_oauth = GoogleOAuthConfig::from_env();

        let user_repo = PostgresUserRepository::new(pool.clone());
        let expense_repo = PostgresExpenseRepository::new(pool.clone());
        let group_repo = PostgresGroupRepository::new(pool.clone());
        let settlement_repo = PostgresSettlementRepository::new(pool.clone());

        let mailer = Mailer::new(128);

        let user_service = Arc::new(UserService::new(
            user_repo.clone(),
            cache.clone(),
            argon2_arc.clone(),
            jwt_config.clone(),
            mailer,
        ));

        let group_service = Arc::new(GroupService::new(
            group_repo.clone(),
            expense_repo.clone(),
            settlement_repo.clone(),
            user_repo.clone(),
            cache.clone(),
        ));

        let expense_service = Arc::new(ExpenseService::new(expense_repo.clone(), group_repo.clone(), cache.clone()));

        let settlement_service =
            Arc::new(SettlementService::new(settlement_repo.clone(), group_repo.clone(), cache.clone()));

        Self {
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
        }
    }

    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        self.db_config.migrate(&self.db_pool).await
    }
}
