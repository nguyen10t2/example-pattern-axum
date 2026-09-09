use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

impl PaginationQuery {
    /// Trang hiện tại (tối thiểu 1).
    #[must_use]
    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }

    /// Số dòng mỗi trang (kẹp 1–100).
    #[must_use]
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(20).clamp(1, 100)
    }

    /// Vị trí bắt đầu đọc (`(page-1) * limit`).
    #[must_use]
    pub fn offset(&self) -> i64 {
        (self.page() - 1) * self.limit()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub limit: i64,
    pub page: i64,
    pub total: i64,
    #[serde(rename = "totalPages")]
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub meta: PaginationMeta,
}

impl<T> PaginatedResponse<T> {
    /// Gom items + meta phân trang (tổng số trang tính ceil chính xác).
    #[must_use]
    pub const fn new(items: Vec<T>, total: i64, page: i64, limit: i64) -> Self {
        // `total` is a row count, never negative: exact integer ceil without float casts.
        let total_pages = if limit > 0 && total > 0 { (total - 1) / limit + 1 } else { 0 };

        Self { items, meta: PaginationMeta { limit, page, total, total_pages } }
    }
}
