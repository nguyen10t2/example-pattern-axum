use super::{AppError, BusinessError};

/// Map a unique constraint violation (Postgres 23505) to `BusinessError`.
///
/// Takes a list of `(constraint_pattern, error_message)` pairs.
/// If the DB error is a unique violation and its constraint name contains
/// a pattern, returns the corresponding business error. Otherwise falls
/// back to `AppError::from(err)`.
///
/// # Example
///
/// ```ignore
/// let created = R::create(pool, &entity).await.map_err(|err| {
///     map_unique_violation(err, &[("email", &entity.email)])
/// })?;
/// ```
#[must_use]
pub fn map_unique_violation(err: sqlx::Error, mappings: &[(&str, &str)]) -> AppError {
    match &err {
        sqlx::Error::Database(db_err) if db_err.code().is_some_and(|c| c == "23505") => {
            let constraint = db_err.constraint().unwrap_or_default();
            for (pattern, value) in mappings {
                if constraint.contains(pattern) {
                    return AppError::Business(BusinessError::DuplicateField {
                        field: pattern.to_string(),
                        value: (*value).to_string(),
                    });
                }
            }
            AppError::from(err)
        }
        _ => AppError::from(err),
    }
}
