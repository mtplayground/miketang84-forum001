use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use sqlx::{query_as, PgPool};
use tracing::error;

use crate::{
    auth::MaybeCurrentUser,
    models::Category,
    pagination::{Pagination, PaginationQuery, DEFAULT_PAGE_SIZE},
    state::AppState,
    templates::{CategoryDetailTemplate, CategoryIndexItem, CategoryIndexTemplate, CategoryThreadListItem},
    threads::ThreadRepository,
};

#[derive(Clone, sqlx::FromRow)]
pub(crate) struct CategoryWithCountsRow {
    pub(crate) id: i64,
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) position: i32,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
    pub(crate) thread_count: i64,
    pub(crate) post_count: i64,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct CategoryRepository {
    db_pool: PgPool,
}

#[allow(dead_code)]
impl CategoryRepository {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn list_all(&self) -> Result<Vec<Category>, sqlx::Error> {
        query_as::<_, Category>(
            r#"
            SELECT id, slug, name, description, position, created_at
            FROM categories
            ORDER BY position ASC, id ASC
            "#,
        )
        .fetch_all(&self.db_pool)
        .await
    }

    pub async fn find_by_slug(&self, slug: &str) -> Result<Option<Category>, sqlx::Error> {
        query_as::<_, Category>(
            r#"
            SELECT id, slug, name, description, position, created_at
            FROM categories
            WHERE slug = $1
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.db_pool)
        .await
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<Category>, sqlx::Error> {
        query_as::<_, Category>(
            r#"
            SELECT id, slug, name, description, position, created_at
            FROM categories
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await
    }

    pub async fn list_all_with_counts(&self) -> Result<Vec<CategoryWithCountsRow>, sqlx::Error> {
        query_as::<_, CategoryWithCountsRow>(
            r#"
            SELECT
                c.id,
                c.slug,
                c.name,
                c.description,
                c.position,
                c.created_at,
                COALESCE(thread_counts.thread_count, 0) AS thread_count,
                COALESCE(post_counts.post_count, 0) AS post_count
            FROM categories c
            LEFT JOIN (
                SELECT category_id, COUNT(*)::bigint AS thread_count
                FROM threads
                WHERE is_deleted = FALSE
                GROUP BY category_id
            ) AS thread_counts ON thread_counts.category_id = c.id
            LEFT JOIN (
                SELECT t.category_id, COUNT(*)::bigint AS post_count
                FROM threads t
                JOIN posts p ON p.thread_id = t.id
                WHERE t.is_deleted = FALSE AND p.is_deleted = FALSE
                GROUP BY t.category_id
            ) AS post_counts ON post_counts.category_id = c.id
            ORDER BY c.position ASC, c.id ASC
            "#,
        )
        .fetch_all(&self.db_pool)
        .await
    }

    pub async fn find_by_slug_with_counts(
        &self,
        slug: &str,
    ) -> Result<Option<CategoryWithCountsRow>, sqlx::Error> {
        query_as::<_, CategoryWithCountsRow>(
            r#"
            SELECT
                c.id,
                c.slug,
                c.name,
                c.description,
                c.position,
                c.created_at,
                COALESCE(thread_counts.thread_count, 0) AS thread_count,
                COALESCE(post_counts.post_count, 0) AS post_count
            FROM categories c
            LEFT JOIN (
                SELECT category_id, COUNT(*)::bigint AS thread_count
                FROM threads
                WHERE is_deleted = FALSE
                GROUP BY category_id
            ) AS thread_counts ON thread_counts.category_id = c.id
            LEFT JOIN (
                SELECT t.category_id, COUNT(*)::bigint AS post_count
                FROM threads t
                JOIN posts p ON p.thread_id = t.id
                WHERE t.is_deleted = FALSE AND p.is_deleted = FALSE
                GROUP BY t.category_id
            ) AS post_counts ON post_counts.category_id = c.id
            WHERE c.slug = $1
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.db_pool)
        .await
    }
}

pub async fn list_categories(
    State(state): State<AppState>,
    current_user: MaybeCurrentUser,
) -> Response {
    let repository = CategoryRepository::new(state.db_pool.clone());

    let categories = match repository.list_all_with_counts().await {
        Ok(categories) => categories,
        Err(db_error) => {
            error!(error = %db_error, "failed to list categories");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let category_items = categories
        .into_iter()
        .map(CategoryIndexItem::from_category_with_counts)
        .collect();

    render_html(
        CategoryIndexTemplate {
            page_title: "Categories",
            categories: category_items,
            is_authenticated: current_user.is_authenticated(),
        },
        StatusCode::OK,
        "failed to render category index template",
    )
}

pub async fn show_category(
    State(state): State<AppState>,
    current_user: MaybeCurrentUser,
    Path(slug): Path<String>,
    Query(pagination_query): Query<PaginationQuery>,
) -> Response {
    let repository = CategoryRepository::new(state.db_pool.clone());
    let thread_repository = ThreadRepository::new(state.db_pool.clone());

    let category = match repository.find_by_slug_with_counts(&slug).await {
        Ok(Some(category)) => category,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, slug, "failed to load category detail");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let total_threads = match thread_repository.count_by_category(category.id).await {
        Ok(total_threads) => total_threads,
        Err(db_error) => {
            error!(error = %db_error, category_id = category.id, "failed to count category threads");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let pagination = Pagination::new(
        pagination_query.requested_page(),
        DEFAULT_PAGE_SIZE,
        total_threads,
    );

    let threads = match thread_repository
        .list_by_category_page(category.id, pagination.per_page, pagination.offset())
        .await
    {
        Ok(threads) => threads,
        Err(db_error) => {
            error!(error = %db_error, category_id = category.id, "failed to list category threads");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let thread_items = threads
        .into_iter()
        .map(CategoryThreadListItem::from_thread)
        .collect();

    render_html(
        CategoryDetailTemplate::from_category_with_counts(
            category,
            current_user.is_authenticated(),
            thread_items,
            pagination,
        ),
        StatusCode::OK,
        "failed to render category detail template",
    )
}

fn render_html<T>(template: T, status: StatusCode, render_error_message: &'static str) -> Response
where
    T: Template,
{
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(error = %render_error, render_error_message);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
