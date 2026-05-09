use askama::Template;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use html_escape::encode_safe;
use serde::Deserialize;
use sqlx::{query_as, query_scalar, PgPool};
use tracing::error;

use crate::{
    auth::MaybeCurrentUser,
    pagination::{Pagination, DEFAULT_PAGE_SIZE},
    state::AppState,
    templates::{SearchResultItem, SearchTemplate},
};

const SEARCH_QUERY_MAX_LENGTH: usize = 200;

#[derive(Debug, Default, Deserialize)]
pub struct SearchPageQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
}

impl SearchPageQuery {
    fn requested_page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }
}

#[derive(Clone)]
struct SearchRepository {
    db_pool: PgPool,
}

#[derive(sqlx::FromRow)]
struct SearchResultRow {
    kind: String,
    thread_id: i64,
    thread_slug: String,
    thread_title: String,
    snippet: String,
    thread_page: i64,
}

impl SearchRepository {
    fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    async fn count_results(&self, query: &str) -> Result<i64, sqlx::Error> {
        query_scalar::<_, i64>(
            r#"
            WITH search_query AS (
                SELECT websearch_to_tsquery('pg_catalog.english', $1) AS q
            )
            SELECT COUNT(*)::bigint
            FROM (
                SELECT 1
                FROM threads t
                CROSS JOIN search_query sq
                WHERE t.is_deleted = FALSE
                  AND t.title_tsv @@ sq.q

                UNION ALL

                SELECT 1
                FROM posts p
                JOIN threads t ON t.id = p.thread_id
                CROSS JOIN search_query sq
                WHERE p.is_deleted = FALSE
                  AND t.is_deleted = FALSE
                  AND p.body_tsv @@ sq.q
            ) AS search_results
            "#,
        )
        .bind(query)
        .fetch_one(&self.db_pool)
        .await
    }

    async fn search_page(
        &self,
        query: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SearchResultRow>, sqlx::Error> {
        query_as::<_, SearchResultRow>(
            r#"
            WITH search_query AS (
                SELECT websearch_to_tsquery('pg_catalog.english', $1) AS q
            ),
            post_numbered AS (
                SELECT
                    p.id,
                    p.thread_id,
                    p.body_md,
                    p.body_tsv,
                    p.created_at,
                    ROW_NUMBER() OVER (
                        PARTITION BY p.thread_id
                        ORDER BY p.created_at ASC, p.id ASC
                    ) AS post_number
                FROM posts p
                WHERE p.is_deleted = FALSE
            ),
            search_results AS (
                SELECT
                    'thread'::text AS kind,
                    t.id AS thread_id,
                    t.slug AS thread_slug,
                    t.title AS thread_title,
                    ts_headline(
                        'pg_catalog.english',
                        t.title,
                        sq.q,
                        'StartSel=__ZCSTART__, StopSel=__ZCEND__, MaxWords=12, MinWords=3'
                    ) AS snippet,
                    1::bigint AS thread_page,
                    ts_rank(t.title_tsv, sq.q) AS rank,
                    t.last_activity_at AS sort_at
                FROM threads t
                CROSS JOIN search_query sq
                WHERE t.is_deleted = FALSE
                  AND t.title_tsv @@ sq.q

                UNION ALL

                SELECT
                    'post'::text AS kind,
                    pn.thread_id,
                    t.slug AS thread_slug,
                    t.title AS thread_title,
                    ts_headline(
                        'pg_catalog.english',
                        pn.body_md,
                        sq.q,
                        'StartSel=__ZCSTART__, StopSel=__ZCEND__, MaxFragments=2, MinWords=5, MaxWords=18'
                    ) AS snippet,
                    CASE
                        WHEN pn.post_number = 1 THEN 1::bigint
                        ELSE ((pn.post_number - 2) / $2) + 1
                    END AS thread_page,
                    ts_rank(pn.body_tsv, sq.q) AS rank,
                    pn.created_at AS sort_at
                FROM post_numbered pn
                JOIN threads t ON t.id = pn.thread_id
                CROSS JOIN search_query sq
                WHERE t.is_deleted = FALSE
                  AND pn.body_tsv @@ sq.q
            )
            SELECT kind, thread_id, thread_slug, thread_title, snippet, thread_page
            FROM search_results
            ORDER BY rank DESC, sort_at DESC, thread_id DESC, kind ASC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(query)
        .bind(DEFAULT_PAGE_SIZE)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db_pool)
        .await
    }
}

pub async fn show_search(
    State(state): State<AppState>,
    current_user: MaybeCurrentUser,
    Query(query): Query<SearchPageQuery>,
) -> Response {
    let requested_page = query.requested_page();
    let raw_query = query.q.unwrap_or_default();
    let trimmed_query = raw_query.trim().to_owned();
    let is_authenticated = current_user.is_authenticated();

    if trimmed_query.is_empty() {
        return render_search_page(
            SearchTemplate::new(
                String::new(),
                Vec::new(),
                Pagination::new(1, DEFAULT_PAGE_SIZE, 0),
                None,
                false,
                is_authenticated,
            ),
            StatusCode::OK,
        );
    }

    if trimmed_query.chars().count() > SEARCH_QUERY_MAX_LENGTH {
        return render_search_page(
            SearchTemplate::new(
                trimmed_query,
                Vec::new(),
                Pagination::new(1, DEFAULT_PAGE_SIZE, 0),
                Some(format!(
                    "Search queries must be at most {SEARCH_QUERY_MAX_LENGTH} characters."
                )),
                true,
                is_authenticated,
            ),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }

    let repository = SearchRepository::new(state.db_pool.clone());
    let total_results = match repository.count_results(&trimmed_query).await {
        Ok(total_results) => total_results,
        Err(db_error) => {
            error!(error = %db_error, query = trimmed_query, "failed to count search results");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let pagination = Pagination::new(requested_page, DEFAULT_PAGE_SIZE, total_results);
    let result_rows = match repository
        .search_page(&trimmed_query, pagination.per_page, pagination.offset())
        .await
    {
        Ok(result_rows) => result_rows,
        Err(db_error) => {
            error!(error = %db_error, query = trimmed_query, "failed to fetch search results");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let results = result_rows
        .into_iter()
        .map(SearchResultItem::from_row)
        .collect::<Vec<_>>();

    render_search_page(
        SearchTemplate::new(
            trimmed_query,
            results,
            pagination,
            None,
            true,
            is_authenticated,
        ),
        StatusCode::OK,
    )
}

impl SearchResultItem {
    fn from_row(row: SearchResultRow) -> Self {
        let destination = if row.kind == "post" {
            format!("/t/{}-{}?page={}", row.thread_id, row.thread_slug, row.thread_page)
        } else {
            format!("/t/{}-{}", row.thread_id, row.thread_slug)
        };

        let (kind_label, destination_label, context) = if row.kind == "post" {
            (
                "Post Match",
                "Open Matching Post",
                format!("In thread {}", row.thread_title),
            )
        } else {
            (
                "Thread Match",
                "Open Thread",
                "Matched in the thread title".to_owned(),
            )
        };

        Self {
            kind_label,
            title: row.thread_title,
            snippet_html: render_search_snippet(&row.snippet),
            destination,
            destination_label,
            context,
        }
    }
}

fn render_search_snippet(snippet: &str) -> String {
    encode_safe(snippet)
        .replace("__ZCSTART__", "<mark>")
        .replace("__ZCEND__", "</mark>")
}

fn render_search_page(template: SearchTemplate, status: StatusCode) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(error = %render_error, "failed to render search template");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
