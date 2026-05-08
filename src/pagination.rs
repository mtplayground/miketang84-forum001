use serde::Deserialize;

pub const DEFAULT_PAGE_SIZE: i64 = 10;

#[derive(Debug, Default, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
}

impl PaginationQuery {
    pub fn requested_page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }
}

#[derive(Debug, Clone)]
pub struct Pagination {
    pub current_page: i64,
    pub per_page: i64,
    pub total_items: i64,
    pub total_pages: i64,
    pub previous_page: Option<i64>,
    pub next_page: Option<i64>,
}

impl Pagination {
    pub fn new(requested_page: i64, per_page: i64, total_items: i64) -> Self {
        let total_pages = ((total_items + per_page - 1) / per_page).max(1);
        let current_page = requested_page.clamp(1, total_pages);
        let previous_page = (current_page > 1).then_some(current_page - 1);
        let next_page = (current_page < total_pages).then_some(current_page + 1);

        Self {
            current_page,
            per_page,
            total_items,
            total_pages,
            previous_page,
            next_page,
        }
    }

    pub fn offset(&self) -> i64 {
        (self.current_page - 1) * self.per_page
    }
}
