use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

impl PaginationQuery {
    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(20).clamp(1, 100)
    }

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
    pub fn new(items: Vec<T>, total: i64, page: i64, limit: i64) -> Self {
        let total_pages = if limit > 0 { (total as f64 / limit as f64).ceil() as i64 } else { 0 };

        Self { items, meta: PaginationMeta { limit, page, total, total_pages } }
    }
}
