use chrono::{DateTime, Utc};
use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use sqlx::{query, query_as, query_scalar, PgPool};
use tracing::error;

use crate::{
    auth::{CurrentUser, MaybeCurrentUser},
    categories::CategoryRepository,
    markdown::render_markdown,
    models::Thread,
    state::AppState,
    templates::{CreateThreadErrors, CreateThreadFormValues, CreateThreadTemplate, ThreadPageTemplate},
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

#[derive(sqlx::FromRow)]
struct ThreadPageRow {
    category_slug: String,
    category_name: String,
    title: String,
    opening_post_html: String,
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

    pub async fn list_by_category(&self, category_id: i64) -> Result<Vec<Thread>, sqlx::Error> {
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
            WHERE category_id = $1
            ORDER BY is_pinned DESC, last_activity_at DESC, id DESC
            "#,
        )
        .bind(category_id)
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

    Redirect::to(&format!("/c/{}/t/{}", category.slug, thread.slug)).into_response()
}

pub async fn show_thread(
    State(state): State<AppState>,
    current_user: MaybeCurrentUser,
    Path((category_slug, thread_slug)): Path<(String, String)>,
) -> Response {
    let thread_page = match fetch_thread_page(&state.db_pool, &category_slug, &thread_slug).await {
        Ok(Some(thread_page)) => thread_page,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(
                error = %db_error,
                category_slug,
                thread_slug,
                "failed to load thread page"
            );
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    render_thread_page(
        ThreadPageTemplate::new(
            thread_page.category_slug,
            thread_page.category_name,
            thread_page.title,
            thread_page.opening_post_html,
            current_user.is_authenticated(),
        ),
        StatusCode::OK,
    )
}

async fn fetch_thread_page(
    db_pool: &PgPool,
    category_slug: &str,
    thread_slug: &str,
) -> Result<Option<ThreadPageRow>, sqlx::Error> {
    query_as::<_, ThreadPageRow>(
        r#"
        SELECT
            c.slug AS category_slug,
            c.name AS category_name,
            t.title,
            COALESCE(opening_post.body_html, '') AS opening_post_html
        FROM threads t
        JOIN categories c ON c.id = t.category_id
        LEFT JOIN LATERAL (
            SELECT body_html
            FROM posts
            WHERE thread_id = t.id AND is_deleted = FALSE
            ORDER BY created_at ASC, id ASC
            LIMIT 1
        ) AS opening_post ON TRUE
        WHERE c.slug = $1 AND t.slug = $2 AND t.is_deleted = FALSE
        "#,
    )
    .bind(category_slug)
    .bind(thread_slug)
    .fetch_optional(db_pool)
    .await
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

fn render_create_thread_page(template: CreateThreadTemplate, status: StatusCode) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(error = %render_error, "failed to render create-thread template");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn render_thread_page(template: ThreadPageTemplate, status: StatusCode) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(error = %render_error, "failed to render thread page template");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
