use super::{AppError, BusinessError};

/// Map lỗi trùng unique (Postgres 23505) sang `BusinessError`.
///
/// Nhận danh sách cặp `(mẫu-tên-constraint, message)`; không khớp thì trả `AppError::from(err)`.
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
