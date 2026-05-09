use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use sqlx::{query, query_as, PgPool};
use tracing::error;

use crate::{
    categories::CategoryRepository,
    models::Category,
    state::AppState,
    templates::{
        AdminCategoryEditTemplate, AdminCategoryFormErrors, AdminCategoryFormValues,
        AdminCategoryListItem, AdminCategoryListTemplate,
    },
    threads::ThreadRepository,
};

const CATEGORY_NAME_MIN_LENGTH: usize = 2;
const CATEGORY_NAME_MAX_LENGTH: usize = 80;
const CATEGORY_DESCRIPTION_MAX_LENGTH: usize = 500;

#[derive(Default, serde::Deserialize, Clone)]
pub struct AdminCategoryForm {
    pub name: String,
    pub description: String,
    pub position: String,
}

#[derive(Default, serde::Deserialize)]
pub struct CategoryPositionForm {
    pub position: String,
}

#[derive(Default, serde::Deserialize)]
pub struct ThreadReturnForm {
    pub return_to: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(admin_home))
        .route("/categories", get(list_categories).post(create_category))
        .route(
            "/categories/{category_id}/edit",
            get(get_edit_category).post(post_edit_category),
        )
        .route("/categories/{category_id}/position", post(reorder_category))
        .route("/categories/{category_id}/delete", post(delete_category))
        .route("/threads/{thread_id}/pin", post(toggle_thread_pin))
        .route("/threads/{thread_id}/lock", post(toggle_thread_lock))
        .fallback(admin_not_found)
}

async fn admin_home() -> Redirect {
    Redirect::to("/admin/categories")
}

pub async fn list_categories(State(state): State<AppState>) -> Response {
    let categories = match load_admin_category_items(&state.db_pool).await {
        Ok(categories) => categories,
        Err(db_error) => {
            error!(error = %db_error, "failed to load admin category list");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let default_position = (categories.len() + 1).to_string();
    render_html(
        AdminCategoryListTemplate::new(
            categories,
            AdminCategoryFormValues {
                position: default_position,
                ..AdminCategoryFormValues::default()
            },
            AdminCategoryFormErrors::default(),
        ),
        StatusCode::OK,
        "failed to render admin category list",
    )
}

pub async fn create_category(
    State(state): State<AppState>,
    Form(form): Form<AdminCategoryForm>,
) -> Response {
    let categories = match load_admin_category_rows(&state.db_pool).await {
        Ok(categories) => categories,
        Err(db_error) => {
            error!(error = %db_error, "failed to load categories for create");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let form_values = AdminCategoryFormValues {
        name: form.name.trim().to_owned(),
        description: form.description.trim().to_owned(),
        position: form.position.trim().to_owned(),
    };

    let desired_position = match validate_category_form(&form_values, categories.len() + 1) {
        Ok(position) => position,
        Err(errors) => {
            let category_items = match load_admin_category_items(&state.db_pool).await {
                Ok(items) => items,
                Err(db_error) => {
                    error!(error = %db_error, "failed to reload categories after create validation");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

            return render_html(
                AdminCategoryListTemplate::new(
                    category_items,
                    form_values,
                    errors,
                ),
                StatusCode::UNPROCESSABLE_ENTITY,
                "failed to render admin category create errors",
            );
        }
    };

    let slug = match generate_unique_category_slug(&state.db_pool, &form_values.name, None).await {
        Ok(slug) => slug,
        Err(db_error) => {
            error!(error = %db_error, "failed to generate category slug");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut transaction = match state.db_pool.begin().await {
        Ok(transaction) => transaction,
        Err(db_error) => {
            error!(error = %db_error, "failed to start category create transaction");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let category = match query_as::<_, Category>(
        r#"
        INSERT INTO categories (slug, name, description, position)
        VALUES ($1, $2, $3, $4)
        RETURNING id, slug, name, description, position, created_at
        "#,
    )
    .bind(&slug)
    .bind(&form_values.name)
    .bind(&form_values.description)
    .bind(i32::try_from(categories.len() + 1).unwrap_or(i32::MAX))
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(category) => category,
        Err(db_error) => {
            error!(error = %db_error, "failed to insert category");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut ordered_ids = categories.into_iter().map(|category| category.id).collect::<Vec<_>>();
    ordered_ids.insert(desired_position - 1, category.id);

    if let Err(db_error) = apply_category_order(&mut transaction, &ordered_ids).await {
        error!(error = %db_error, "failed to reorder categories after create");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Err(db_error) = transaction.commit().await {
        error!(error = %db_error, "failed to commit category create");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Redirect::to("/admin/categories").into_response()
}

pub async fn get_edit_category(
    State(state): State<AppState>,
    Path(category_id): Path<i64>,
) -> Response {
    let repository = CategoryRepository::new(state.db_pool.clone());
    let category = match repository.find_by_id(category_id).await {
        Ok(Some(category)) => category,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, category_id, "failed to load category edit page");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    render_html(
        AdminCategoryEditTemplate::new(
            category.id,
            AdminCategoryFormValues {
                name: category.name,
                description: category.description,
                position: category.position.to_string(),
            },
            AdminCategoryFormErrors::default(),
        ),
        StatusCode::OK,
        "failed to render admin category edit",
    )
}

pub async fn post_edit_category(
    State(state): State<AppState>,
    Path(category_id): Path<i64>,
    Form(form): Form<AdminCategoryForm>,
) -> Response {
    let repository = CategoryRepository::new(state.db_pool.clone());
    let existing = match repository.find_by_id(category_id).await {
        Ok(Some(category)) => category,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, category_id, "failed to load category for edit");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let ordered_categories = match repository.list_all().await {
        Ok(categories) => categories,
        Err(db_error) => {
            error!(error = %db_error, category_id, "failed to list categories for edit");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let form_values = AdminCategoryFormValues {
        name: form.name.trim().to_owned(),
        description: form.description.trim().to_owned(),
        position: form.position.trim().to_owned(),
    };

    let desired_position = match validate_category_form(&form_values, ordered_categories.len()) {
        Ok(position) => position,
        Err(errors) => {
            return render_html(
                AdminCategoryEditTemplate::new(category_id, form_values, errors),
                StatusCode::UNPROCESSABLE_ENTITY,
                "failed to render admin category edit errors",
            );
        }
    };

    let slug = match generate_unique_category_slug(&state.db_pool, &form_values.name, Some(category_id)).await
    {
        Ok(slug) => slug,
        Err(db_error) => {
            error!(error = %db_error, category_id, "failed to generate category slug for edit");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut transaction = match state.db_pool.begin().await {
        Ok(transaction) => transaction,
        Err(db_error) => {
            error!(error = %db_error, category_id, "failed to start category edit transaction");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Err(db_error) = query(
        r#"
        UPDATE categories
        SET slug = $2, name = $3, description = $4
        WHERE id = $1
        "#,
    )
    .bind(category_id)
    .bind(&slug)
    .bind(&form_values.name)
    .bind(&form_values.description)
    .execute(&mut *transaction)
    .await
    {
        error!(error = %db_error, category_id, "failed to update category");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let mut ordered_ids = ordered_categories
        .into_iter()
        .map(|category| category.id)
        .filter(|id| *id != category_id)
        .collect::<Vec<_>>();
    ordered_ids.insert(desired_position - 1, category_id);

    if let Err(db_error) = apply_category_order(&mut transaction, &ordered_ids).await {
        error!(error = %db_error, category_id, "failed to reorder categories after edit");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Err(db_error) = transaction.commit().await {
        error!(error = %db_error, category_id, "failed to commit category edit");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let _ = existing;
    Redirect::to("/admin/categories").into_response()
}

pub async fn reorder_category(
    State(state): State<AppState>,
    Path(category_id): Path<i64>,
    Form(form): Form<CategoryPositionForm>,
) -> Response {
    let repository = CategoryRepository::new(state.db_pool.clone());
    let category = match repository.find_by_id(category_id).await {
        Ok(Some(category)) => category,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, category_id, "failed to load category for reorder");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let ordered_categories = match repository.list_all_with_counts().await {
        Ok(categories) => categories,
        Err(db_error) => {
            error!(error = %db_error, category_id, "failed to list categories for reorder");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let form_values = AdminCategoryFormValues {
        name: category.name,
        description: category.description,
        position: form.position.trim().to_owned(),
    };
    let total_categories = ordered_categories.len();

    let desired_position = match parse_position(&form_values.position, total_categories) {
        Ok(position) => position,
        Err(message) => {
            return render_html(
                AdminCategoryListTemplate::new(
                    ordered_categories
                        .into_iter()
                        .map(AdminCategoryListItem::from_row)
                        .collect(),
                    AdminCategoryFormValues {
                        position: (total_categories + 1).to_string(),
                        ..AdminCategoryFormValues::default()
                    },
                    AdminCategoryFormErrors {
                        general: Some(message),
                        ..AdminCategoryFormErrors::default()
                    },
                ),
                StatusCode::UNPROCESSABLE_ENTITY,
                "failed to render admin category reorder errors",
            );
        }
    };

    let mut transaction = match state.db_pool.begin().await {
        Ok(transaction) => transaction,
        Err(db_error) => {
            error!(error = %db_error, category_id, "failed to start reorder transaction");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut ordered_ids = ordered_categories
        .into_iter()
        .map(|category| category.id)
        .filter(|id| *id != category_id)
        .collect::<Vec<_>>();
    ordered_ids.insert(desired_position - 1, category_id);

    if let Err(db_error) = apply_category_order(&mut transaction, &ordered_ids).await {
        error!(error = %db_error, category_id, "failed to apply category reorder");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Err(db_error) = transaction.commit().await {
        error!(error = %db_error, category_id, "failed to commit category reorder");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Redirect::to("/admin/categories").into_response()
}

pub async fn delete_category(
    State(state): State<AppState>,
    Path(category_id): Path<i64>,
) -> Response {
    let repository = CategoryRepository::new(state.db_pool.clone());
    let ordered_categories = match repository.list_all_with_counts().await {
        Ok(categories) => categories,
        Err(db_error) => {
            error!(error = %db_error, category_id, "failed to list categories for delete");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if !ordered_categories.iter().any(|category| category.id == category_id) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let mut transaction = match state.db_pool.begin().await {
        Ok(transaction) => transaction,
        Err(db_error) => {
            error!(error = %db_error, category_id, "failed to start delete transaction");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Err(db_error) = query("DELETE FROM categories WHERE id = $1")
        .bind(category_id)
        .execute(&mut *transaction)
        .await
    {
        let total_categories = ordered_categories.len();
        error!(error = %db_error, category_id, "failed to delete category");
        return render_html(
            AdminCategoryListTemplate::new(
                ordered_categories
                    .into_iter()
                    .map(AdminCategoryListItem::from_row)
                    .collect(),
                AdminCategoryFormValues {
                    position: total_categories.to_string(),
                    ..AdminCategoryFormValues::default()
                },
                AdminCategoryFormErrors {
                    general: Some(
                        "This category could not be deleted. Remove its threads first.".to_owned(),
                    ),
                    ..AdminCategoryFormErrors::default()
                },
            ),
            StatusCode::CONFLICT,
            "failed to render category delete conflict",
        );
    }

    let remaining_ids = ordered_categories
        .into_iter()
        .map(|category| category.id)
        .filter(|id| *id != category_id)
        .collect::<Vec<_>>();

    if let Err(db_error) = apply_category_order(&mut transaction, &remaining_ids).await {
        error!(error = %db_error, category_id, "failed to normalize order after delete");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Err(db_error) = transaction.commit().await {
        error!(error = %db_error, category_id, "failed to commit category delete");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Redirect::to("/admin/categories").into_response()
}

pub async fn toggle_thread_pin(
    State(state): State<AppState>,
    Path(thread_id): Path<i64>,
    Form(form): Form<ThreadReturnForm>,
) -> Response {
    toggle_thread_flag(&state.db_pool, thread_id, "is_pinned", form.return_to).await
}

pub async fn toggle_thread_lock(
    State(state): State<AppState>,
    Path(thread_id): Path<i64>,
    Form(form): Form<ThreadReturnForm>,
) -> Response {
    toggle_thread_flag(&state.db_pool, thread_id, "is_locked", form.return_to).await
}

async fn load_admin_category_rows(db_pool: &PgPool) -> Result<Vec<Category>, sqlx::Error> {
    CategoryRepository::new(db_pool.clone()).list_all().await
}

async fn load_admin_category_items(db_pool: &PgPool) -> Result<Vec<AdminCategoryListItem>, sqlx::Error> {
    Ok(CategoryRepository::new(db_pool.clone())
        .list_all_with_counts()
        .await?
        .into_iter()
        .map(AdminCategoryListItem::from_row)
        .collect())
}

fn validate_category_form(
    form: &AdminCategoryFormValues,
    max_position: usize,
) -> Result<usize, AdminCategoryFormErrors> {
    let mut errors = AdminCategoryFormErrors::default();

    if form.name.len() < CATEGORY_NAME_MIN_LENGTH {
        errors.name = Some(format!(
            "Name must be at least {CATEGORY_NAME_MIN_LENGTH} characters."
        ));
    } else if form.name.len() > CATEGORY_NAME_MAX_LENGTH {
        errors.name = Some(format!(
            "Name must be at most {CATEGORY_NAME_MAX_LENGTH} characters."
        ));
    }

    if form.description.chars().count() > CATEGORY_DESCRIPTION_MAX_LENGTH {
        errors.general = Some(format!(
            "Description must be at most {CATEGORY_DESCRIPTION_MAX_LENGTH} characters."
        ));
    }

    let position = match parse_position(&form.position, max_position) {
        Ok(position) => position,
        Err(message) => {
            errors.position = Some(message);
            1
        }
    };

    if errors.is_empty() {
        Ok(position)
    } else {
        Err(errors)
    }
}

fn parse_position(raw: &str, max_position: usize) -> Result<usize, String> {
    let position = raw
        .parse::<usize>()
        .map_err(|_| "Position must be a number.".to_owned())?;

    if position == 0 || position > max_position {
        return Err(format!("Position must be between 1 and {max_position}."));
    }

    Ok(position)
}

async fn generate_unique_category_slug(
    db_pool: &PgPool,
    name: &str,
    exclude_category_id: Option<i64>,
) -> Result<String, sqlx::Error> {
    let base_slug = slugify_name(name);
    let mut candidate = base_slug.clone();
    let mut suffix = 2;

    while category_slug_exists(db_pool, &candidate, exclude_category_id).await? {
        candidate = format!("{base_slug}-{suffix}");
        suffix += 1;
    }

    Ok(candidate)
}

async fn category_slug_exists(
    db_pool: &PgPool,
    slug: &str,
    exclude_category_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let query_text = if exclude_category_id.is_some() {
        "SELECT EXISTS(SELECT 1 FROM categories WHERE slug = $1 AND id <> $2)"
    } else {
        "SELECT EXISTS(SELECT 1 FROM categories WHERE slug = $1)"
    };

    let mut query = sqlx::query_scalar::<_, bool>(query_text).bind(slug);

    if let Some(category_id) = exclude_category_id {
        query = query.bind(category_id);
    }

    query.fetch_one(db_pool).await
}

fn slugify_name(name: &str) -> String {
    let mut slug = String::new();
    let mut last_was_hyphen = false;

    for character in name.chars() {
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
        "category".to_owned()
    } else {
        trimmed
    }
}

async fn apply_category_order(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ordered_ids: &[i64],
) -> Result<(), sqlx::Error> {
    for (index, category_id) in ordered_ids.iter().enumerate() {
        query("UPDATE categories SET position = $2 WHERE id = $1")
            .bind(category_id)
            .bind(i32::try_from(index + 1).unwrap_or(i32::MAX))
            .execute(&mut **transaction)
            .await?;
    }

    Ok(())
}

async fn toggle_thread_flag(
    db_pool: &PgPool,
    thread_id: i64,
    column: &'static str,
    return_to: Option<String>,
) -> Response {
    let repository = ThreadRepository::new(db_pool.clone());
    let thread = match repository.find_by_id(thread_id).await {
        Ok(Some(thread)) if !thread.is_deleted => thread,
        Ok(Some(_)) | Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, thread_id, column, "failed to load thread for admin toggle");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let sql = match column {
        "is_pinned" => "UPDATE threads SET is_pinned = NOT is_pinned WHERE id = $1",
        "is_locked" => "UPDATE threads SET is_locked = NOT is_locked WHERE id = $1",
        _ => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if let Err(db_error) = query(sql).bind(thread_id).execute(db_pool).await {
        error!(error = %db_error, thread_id, column, "failed to toggle thread flag");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let destination = return_to.unwrap_or_else(|| format!("/t/{}-{}", thread.id, thread.slug));
    Redirect::to(&destination).into_response()
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

async fn admin_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}
