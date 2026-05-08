use askama::Template;
use axum::{
    extract::{Form, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Utc};
use sqlx::{query, query_as, query_scalar, PgPool};
use tracing::error;

use crate::{
    auth::{CurrentUser, MaybeCurrentUser},
    categories::CategoryRepository,
    markdown::render_markdown,
    models::Thread,
    pagination::{Pagination, PaginationQuery, DEFAULT_PAGE_SIZE},
    state::AppState,
    templates::{
        CreateThreadErrors, CreateThreadFormValues, CreateThreadTemplate, ThreadDetailTemplate,
        ReplyErrors, ReplyFormValues, ThreadPostItem,
    },
};

const THREAD_TITLE_MIN_LENGTH: usize = 3;
const THREAD_TITLE_MAX_LENGTH: usize = 120;
const THREAD_BODY_MAX_LENGTH: usize = 20_000;

#[derive(Clone)]
#[allow(dead_code)]
pub struct ThreadRepository {
    db_pool: PgPool,
}

#[allow(dead_code)]
pub struct CreateThreadParams<'a> {
    pub category_id: i64,
    pub user_id: i64,
    pub title: &'a str,
    pub slug: &'a str,
}

#[derive(Default, serde::Deserialize)]
pub struct CreateThreadForm {
    pub title: String,
    pub body: String,
}

#[derive(Default, serde::Deserialize)]
pub struct ReplyForm {
    pub body: String,
}

#[derive(sqlx::FromRow)]
struct ThreadDetailRow {
    id: i64,
    slug: String,
    title: String,
    category_slug: String,
    category_name: String,
    is_locked: bool,
}

#[derive(sqlx::FromRow)]
struct ThreadPostRow {
    username: String,
    body_html: String,
    created_at: DateTime<Utc>,
    edited_at: Option<DateTime<Utc>>,
}

#[allow(dead_code)]
impl ThreadRepository {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn create(&self, params: CreateThreadParams<'_>) -> Result<Thread, sqlx::Error> {
        query_as::<_, Thread>(
            r#"
            INSERT INTO threads (category_id, user_id, title, slug)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id,
                category_id,
                user_id,
                title,
                slug,
                is_pinned,
                is_locked,
                is_deleted,
                created_at,
                last_activity_at
            "#,
        )
        .bind(params.category_id)
        .bind(params.user_id)
        .bind(params.title)
        .bind(params.slug)
        .fetch_one(&self.db_pool)
        .await
    }

    pub async fn find_by_category_and_slug(
        &self,
        category_id: i64,
        slug: &str,
    ) -> Result<Option<Thread>, sqlx::Error> {
        query_as::<_, Thread>(
            r#"
            SELECT
                id,
                category_id,
                user_id,
                title,
                slug,
                is_pinned,
                is_locked,
                is_deleted,
                created_at,
                last_activity_at
            FROM threads
            WHERE category_id = $1 AND slug = $2
            "#,
        )
        .bind(category_id)
        .bind(slug)
        .fetch_optional(&self.db_pool)
        .await
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<Thread>, sqlx::Error> {
        query_as::<_, Thread>(
            r#"
            SELECT
                id,
                category_id,
                user_id,
                title,
                slug,
                is_pinned,
                is_locked,
                is_deleted,
                created_at,
                last_activity_at
            FROM threads
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await
    }

    pub async fn count_by_category(&self, category_id: i64) -> Result<i64, sqlx::Error> {
        query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)::bigint
            FROM threads
            WHERE category_id = $1 AND is_deleted = FALSE
            "#,
        )
        .bind(category_id)
        .fetch_one(&self.db_pool)
        .await
    }

    pub async fn list_by_category_page(
        &self,
        category_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Thread>, sqlx::Error> {
        query_as::<_, Thread>(
            r#"
            SELECT
                id,
                category_id,
                user_id,
                title,
                slug,
                is_pinned,
                is_locked,
                is_deleted,
                created_at,
                last_activity_at
            FROM threads
            WHERE category_id = $1 AND is_deleted = FALSE
            ORDER BY is_pinned DESC, last_activity_at DESC, id DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(category_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db_pool)
        .await
    }

    pub async fn touch_last_activity(
        &self,
        id: i64,
        last_activity_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        query(
            r#"
            UPDATE threads
            SET last_activity_at = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(last_activity_at)
        .execute(&self.db_pool)
        .await
        .map(|_| ())
    }

    async fn find_detail_by_id(&self, id: i64) -> Result<Option<ThreadDetailRow>, sqlx::Error> {
        query_as::<_, ThreadDetailRow>(
            r#"
            SELECT
                t.id,
                t.slug,
                t.title,
                c.slug AS category_slug,
                c.name AS category_name,
                t.is_locked
            FROM threads t
            JOIN categories c ON c.id = t.category_id
            WHERE t.id = $1 AND t.is_deleted = FALSE
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await
    }

    async fn find_opening_post(&self, thread_id: i64) -> Result<Option<ThreadPostRow>, sqlx::Error> {
        query_as::<_, ThreadPostRow>(
            r#"
            SELECT
                u.username,
                p.body_html,
                p.created_at,
                p.edited_at
            FROM posts p
            JOIN users u ON u.id = p.user_id
            WHERE p.thread_id = $1 AND p.is_deleted = FALSE
            ORDER BY p.created_at ASC, p.id ASC
            LIMIT 1
            "#,
        )
        .bind(thread_id)
        .fetch_optional(&self.db_pool)
        .await
    }

    async fn count_replies(&self, thread_id: i64) -> Result<i64, sqlx::Error> {
        query_scalar::<_, i64>(
            r#"
            SELECT GREATEST(COUNT(*)::bigint - 1, 0)
            FROM posts
            WHERE thread_id = $1 AND is_deleted = FALSE
            "#,
        )
        .bind(thread_id)
        .fetch_one(&self.db_pool)
        .await
    }

    async fn list_replies_page(
        &self,
        thread_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ThreadPostRow>, sqlx::Error> {
        query_as::<_, ThreadPostRow>(
            r#"
            SELECT
                u.username,
                p.body_html,
                p.created_at,
                p.edited_at
            FROM posts p
            JOIN users u ON u.id = p.user_id
            WHERE p.thread_id = $1 AND p.is_deleted = FALSE
            ORDER BY p.created_at ASC, p.id ASC
            OFFSET $2 + 1
            LIMIT $3
            "#,
        )
        .bind(thread_id)
        .bind(offset)
        .bind(limit)
        .fetch_all(&self.db_pool)
        .await
    }
}

pub async fn get_create_thread(
    State(state): State<AppState>,
    CurrentUser(_current_user): CurrentUser,
    Path(category_slug): Path<String>,
) -> Response {
    let category_repository = CategoryRepository::new(state.db_pool.clone());

    let category = match category_repository.find_by_slug(&category_slug).await {
        Ok(Some(category)) => category,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, category_slug, "failed to load category for thread form");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    render_create_thread_page(
        CreateThreadTemplate::new(
            category.slug,
            category.name,
            CreateThreadFormValues::default(),
            CreateThreadErrors::default(),
        ),
        StatusCode::OK,
    )
}

pub async fn post_create_thread(
    State(state): State<AppState>,
    CurrentUser(current_user): CurrentUser,
    Path(category_slug): Path<String>,
    Form(form): Form<CreateThreadForm>,
) -> Response {
    let category_repository = CategoryRepository::new(state.db_pool.clone());

    let category = match category_repository.find_by_slug(&category_slug).await {
        Ok(Some(category)) => category,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(
                error = %db_error,
                category_slug,
                "failed to load category for thread creation"
            );
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let title = form.title.trim().to_owned();
    let body = normalize_body(&form.body);
    let form_values = CreateThreadFormValues {
        title: title.clone(),
        body: body.clone(),
    };
    let errors = validate_create_thread_form(&title, &body);

    if !errors.is_empty() {
        return render_create_thread_page(
            CreateThreadTemplate::new(category.slug, category.name, form_values, errors),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }

    let thread_slug = match generate_unique_thread_slug(&state.db_pool, category.id, &title).await {
        Ok(slug) => slug,
        Err(db_error) => {
            error!(error = %db_error, category_id = category.id, "failed to generate thread slug");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let rendered_body_html = render_markdown(&body);
    let mut transaction = match state.db_pool.begin().await {
        Ok(transaction) => transaction,
        Err(db_error) => {
            error!(error = %db_error, "failed to start thread creation transaction");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let thread = match query_as::<_, Thread>(
        r#"
        INSERT INTO threads (category_id, user_id, title, slug)
        VALUES ($1, $2, $3, $4)
        RETURNING
            id,
            category_id,
            user_id,
            title,
            slug,
            is_pinned,
            is_locked,
            is_deleted,
            created_at,
            last_activity_at
        "#,
    )
    .bind(category.id)
    .bind(current_user.id)
    .bind(&title)
    .bind(&thread_slug)
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(thread) => thread,
        Err(db_error) => {
            error!(error = %db_error, "failed to insert thread");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Err(db_error) = query(
        r#"
        INSERT INTO posts (thread_id, user_id, body_md, body_html)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(thread.id)
    .bind(current_user.id)
    .bind(&body)
    .bind(&rendered_body_html)
    .execute(&mut *transaction)
    .await
    {
        error!(error = %db_error, thread_id = thread.id, "failed to insert opening post");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Err(db_error) = transaction.commit().await {
        error!(error = %db_error, thread_id = thread.id, "failed to commit thread creation");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Redirect::to(&format!("/t/{}-{}", thread.id, thread.slug)).into_response()
}

pub async fn show_thread(
    State(state): State<AppState>,
    current_user: MaybeCurrentUser,
    Path(thread_ref): Path<String>,
    Query(pagination_query): Query<PaginationQuery>,
) -> Response {
    render_thread_detail_page(
        &state,
        current_user.is_authenticated(),
        &thread_ref,
        pagination_query.requested_page(),
        ReplyFormValues::default(),
        ReplyErrors::default(),
        StatusCode::OK,
    )
    .await
}

pub async fn post_reply_to_thread(
    State(state): State<AppState>,
    CurrentUser(current_user): CurrentUser,
    Path(thread_ref): Path<String>,
    Form(form): Form<ReplyForm>,
) -> Response {
    let (thread_id, requested_slug) = match parse_thread_ref(&thread_ref) {
        Some(parts) => parts,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let repository = ThreadRepository::new(state.db_pool.clone());
    let thread = match repository.find_detail_by_id(thread_id).await {
        Ok(Some(thread)) => thread,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, thread_id, "failed to load thread detail for reply");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if thread.slug != requested_slug {
        return Redirect::to(&format!("/t/{}-{}", thread.id, thread.slug)).into_response();
    }

    let body = normalize_body(&form.body);
    let form_values = ReplyFormValues { body: body.clone() };
    let errors = validate_reply_form(&body, thread.is_locked);

    if !errors.is_empty() {
        let status = if thread.is_locked {
            StatusCode::LOCKED
        } else {
            StatusCode::UNPROCESSABLE_ENTITY
        };

        return render_thread_detail_page(
            &state,
            true,
            &thread_ref,
            1,
            form_values,
            errors,
            status,
        )
        .await;
    }

    let rendered_body_html = render_markdown(&body);
    let now = Utc::now();
    let mut transaction = match state.db_pool.begin().await {
        Ok(transaction) => transaction,
        Err(db_error) => {
            error!(error = %db_error, thread_id, "failed to start reply transaction");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Err(db_error) = query(
        r#"
        INSERT INTO posts (thread_id, user_id, body_md, body_html)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(thread.id)
    .bind(current_user.id)
    .bind(&body)
    .bind(&rendered_body_html)
    .execute(&mut *transaction)
    .await
    {
        error!(error = %db_error, thread_id = thread.id, "failed to insert reply post");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Err(db_error) = query(
        r#"
        UPDATE threads
        SET last_activity_at = $2
        WHERE id = $1
        "#,
    )
    .bind(thread.id)
    .bind(now)
    .execute(&mut *transaction)
    .await
    {
        error!(error = %db_error, thread_id = thread.id, "failed to update thread activity");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Err(db_error) = transaction.commit().await {
        error!(error = %db_error, thread_id = thread.id, "failed to commit reply transaction");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let new_reply_count = match repository.count_replies(thread.id).await {
        Ok(reply_count) => reply_count,
        Err(db_error) => {
            error!(error = %db_error, thread_id = thread.id, "failed to count replies after insert");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let reply_page = ((new_reply_count + DEFAULT_PAGE_SIZE - 1) / DEFAULT_PAGE_SIZE).max(1);
    Redirect::to(&format!("/t/{}-{}?page={reply_page}", thread.id, thread.slug)).into_response()
}

async fn render_thread_detail_page(
    state: &AppState,
    is_authenticated: bool,
    thread_ref: &str,
    requested_page: i64,
    reply_form: ReplyFormValues,
    reply_errors: ReplyErrors,
    status: StatusCode,
) -> Response {
    let (thread_id, requested_slug) = match parse_thread_ref(&thread_ref) {
        Some(parts) => parts,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let repository = ThreadRepository::new(state.db_pool.clone());

    let thread = match repository.find_detail_by_id(thread_id).await {
        Ok(Some(thread)) => thread,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, thread_id, "failed to load thread detail");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if thread.slug != requested_slug {
        return Redirect::to(&format!("/t/{}-{}", thread.id, thread.slug)).into_response();
    }

    let opening_post = match repository.find_opening_post(thread.id).await {
        Ok(Some(opening_post)) => opening_post,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, thread_id = thread.id, "failed to load opening post");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let reply_count = match repository.count_replies(thread.id).await {
        Ok(reply_count) => reply_count,
        Err(db_error) => {
            error!(error = %db_error, thread_id = thread.id, "failed to count thread replies");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let pagination = Pagination::new(requested_page, DEFAULT_PAGE_SIZE, reply_count);

    let replies = match repository
        .list_replies_page(thread.id, pagination.per_page, pagination.offset())
        .await
    {
        Ok(replies) => replies,
        Err(db_error) => {
            error!(error = %db_error, thread_id = thread.id, "failed to load thread replies");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let template = ThreadDetailTemplate::new(
        thread.id,
        thread.slug,
        thread.category_slug,
        thread.category_name,
        thread.title,
        thread_post_item(opening_post),
        replies.into_iter().map(thread_post_item).collect(),
        reply_count,
        pagination,
        is_authenticated,
        thread.is_locked,
        reply_form,
        reply_errors,
    );

    render_thread_page(template, status)
}

pub async fn show_thread_legacy(
    State(state): State<AppState>,
    Path((category_slug, thread_slug)): Path<(String, String)>,
) -> Response {
    let category_repository = CategoryRepository::new(state.db_pool.clone());
    let thread_repository = ThreadRepository::new(state.db_pool.clone());

    let category = match category_repository.find_by_slug(&category_slug).await {
        Ok(Some(category)) => category,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, category_slug, "failed to load category for legacy thread URL");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let thread = match thread_repository
        .find_by_category_and_slug(category.id, &thread_slug)
        .await
    {
        Ok(Some(thread)) if !thread.is_deleted => thread,
        Ok(Some(_)) | Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(
                error = %db_error,
                category_id = category.id,
                thread_slug,
                "failed to load legacy thread URL"
            );
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Redirect::to(&format!("/t/{}-{}", thread.id, thread.slug)).into_response()
}

fn validate_create_thread_form(title: &str, body: &str) -> CreateThreadErrors {
    let mut errors = CreateThreadErrors::default();

    if title.len() < THREAD_TITLE_MIN_LENGTH {
        errors.title = Some(format!(
            "Title must be at least {THREAD_TITLE_MIN_LENGTH} characters."
        ));
    } else if title.len() > THREAD_TITLE_MAX_LENGTH {
        errors.title = Some(format!(
            "Title must be at most {THREAD_TITLE_MAX_LENGTH} characters."
        ));
    }

    if body.trim().is_empty() {
        errors.body = Some("Enter the opening post for your thread.".to_owned());
    } else if body.chars().count() > THREAD_BODY_MAX_LENGTH {
        errors.body = Some(format!(
            "Body must be at most {THREAD_BODY_MAX_LENGTH} characters."
        ));
    }

    errors
}

fn normalize_body(body: &str) -> String {
    body.replace("\r\n", "\n")
}

fn validate_reply_form(body: &str, is_locked: bool) -> ReplyErrors {
    let mut errors = ReplyErrors::default();

    if is_locked {
        errors.general = Some("This thread is locked and cannot receive new replies.".to_owned());
        return errors;
    }

    if body.trim().is_empty() {
        errors.body = Some("Enter a reply before posting.".to_owned());
    } else if body.chars().count() > THREAD_BODY_MAX_LENGTH {
        errors.body = Some(format!(
            "Reply must be at most {THREAD_BODY_MAX_LENGTH} characters."
        ));
    }

    errors
}

async fn generate_unique_thread_slug(
    db_pool: &PgPool,
    category_id: i64,
    title: &str,
) -> Result<String, sqlx::Error> {
    let base_slug = slugify_title(title);
    let mut candidate = base_slug.clone();
    let mut suffix = 2;

    while thread_slug_exists(db_pool, category_id, &candidate).await? {
        candidate = format!("{base_slug}-{suffix}");
        suffix += 1;
    }

    Ok(candidate)
}

async fn thread_slug_exists(
    db_pool: &PgPool,
    category_id: i64,
    slug: &str,
) -> Result<bool, sqlx::Error> {
    query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM threads WHERE category_id = $1 AND slug = $2)",
    )
    .bind(category_id)
    .bind(slug)
    .fetch_one(db_pool)
    .await
}

fn slugify_title(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_hyphen = false;

    for character in title.chars() {
        let lower = character.to_ascii_lowercase();

        if lower.is_ascii_alphanumeric() {
            slug.push(lower);
            last_was_hyphen = false;
        } else if !last_was_hyphen && !slug.is_empty() {
            slug.push('-');
            last_was_hyphen = true;
        }
    }

    let trimmed = slug.trim_matches('-').to_owned();

    if trimmed.is_empty() {
        "thread".to_owned()
    } else {
        trimmed
    }
}

fn parse_thread_ref(thread_ref: &str) -> Option<(i64, String)> {
    let (id, slug) = thread_ref.split_once('-')?;
    let id = id.parse::<i64>().ok()?;

    if slug.is_empty() {
        return None;
    }

    Some((id, slug.to_owned()))
}

fn thread_post_item(row: ThreadPostRow) -> ThreadPostItem {
    ThreadPostItem::new(row.username, row.created_at, row.edited_at, row.body_html)
}

fn render_create_thread_page(template: CreateThreadTemplate, status: StatusCode) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(error = %render_error, "failed to render create-thread template");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn render_thread_page(template: ThreadDetailTemplate, status: StatusCode) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(error = %render_error, "failed to render thread page template");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
