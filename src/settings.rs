use askama::Template;
use axum::{
    extract::{Form, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use sqlx::query_as;
use tracing::error;

use crate::{
    auth::CurrentUser,
    models::User,
    state::AppState,
    templates::{ProfileSettingsErrors, ProfileSettingsFormValues, ProfileSettingsTemplate},
};

const BIO_MAX_LENGTH: usize = 2_000;

#[derive(Debug, Default, Deserialize)]
pub struct ProfileSettingsForm {
    pub bio: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ProfileSettingsQuery {
    pub updated: Option<u8>,
}

pub async fn get_profile_settings(
    CurrentUser(current_user): CurrentUser,
    Query(query): Query<ProfileSettingsQuery>,
) -> Response {
    let saved = query.updated.is_some_and(|updated| updated == 1);

    render_profile_settings_page(
        ProfileSettingsTemplate::new(
            current_user.username.clone(),
            ProfileSettingsFormValues {
                bio: current_user.bio.clone(),
            },
            ProfileSettingsErrors::default(),
            saved,
        ),
        StatusCode::OK,
    )
}

pub async fn post_profile_settings(
    State(state): State<AppState>,
    CurrentUser(current_user): CurrentUser,
    Form(form): Form<ProfileSettingsForm>,
) -> Response {
    let bio = normalize_bio(&form.bio);
    let form_values = ProfileSettingsFormValues { bio: bio.clone() };
    let mut errors = ProfileSettingsErrors::default();

    if bio.chars().count() > BIO_MAX_LENGTH {
        errors.bio = Some(format!("Bio must be at most {BIO_MAX_LENGTH} characters."));
    }

    if !errors.is_empty() {
        return render_profile_settings_page(
            ProfileSettingsTemplate::new(current_user.username, form_values, errors, false),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }

    let updated_user = match update_bio(&state, current_user.id, &bio).await {
        Ok(user) => user,
        Err(db_error) => {
            error!(error = %db_error, user_id = current_user.id, "failed to update profile bio");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Redirect::to(&format!(
        "/settings/profile?updated=1#bio-for-{}",
        updated_user.username
    ))
    .into_response()
}

async fn update_bio(state: &AppState, user_id: i64, bio: &str) -> Result<User, sqlx::Error> {
    query_as::<_, User>(
        r#"
        UPDATE users
        SET bio = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, username, password_hash, role, bio, created_at, updated_at
        "#,
    )
    .bind(bio)
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
}

fn normalize_bio(bio: &str) -> String {
    let normalized = bio.replace("\r\n", "\n");

    if normalized.trim().is_empty() {
        String::new()
    } else {
        normalized
    }
}

fn render_profile_settings_page(template: ProfileSettingsTemplate, status: StatusCode) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(
                error = %render_error,
                "failed to render profile settings template"
            );
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
