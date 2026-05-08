use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use sqlx::query_as;
use tracing::error;

use crate::{auth::MaybeCurrentUser, models::User, state::AppState, templates::ProfileTemplate};

pub async fn show_profile(
    State(state): State<AppState>,
    current_user: MaybeCurrentUser,
    Path(username): Path<String>,
) -> Response {
    let profile_user = match find_user_by_username(&state, &username).await {
        Ok(Some(user)) => user,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(db_error) => {
            error!(error = %db_error, username, "failed to load public profile");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    render_profile_page(
        ProfileTemplate::from_user(profile_user, current_user.is_authenticated(), 0),
        StatusCode::OK,
    )
}

async fn find_user_by_username(
    state: &AppState,
    username: &str,
) -> Result<Option<User>, sqlx::Error> {
    query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, role, bio, created_at, updated_at
        FROM users
        WHERE username = $1
        "#,
    )
    .bind(username)
    .fetch_optional(&state.db_pool)
    .await
}

fn render_profile_page(template: ProfileTemplate, status: StatusCode) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(error = %render_error, "failed to render profile template");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
