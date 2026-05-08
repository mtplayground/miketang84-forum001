use askama::Template;
use axum::{
    extract::{Form, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use sqlx::{query, query_as};
use tower_sessions::Session;
use tracing::error;

use crate::{
    auth::CurrentUser,
    models::User,
    password::{hash_password, verify_password},
    state::AppState,
    templates::{
        PasswordSettingsErrors, PasswordSettingsTemplate, ProfileSettingsErrors,
        ProfileSettingsFormValues, ProfileSettingsTemplate,
    },
};

const BIO_MAX_LENGTH: usize = 2_000;
const PASSWORD_MIN_LENGTH: usize = 8;

#[derive(Debug, Default, Deserialize)]
pub struct ProfileSettingsForm {
    pub bio: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ProfileSettingsQuery {
    pub updated: Option<u8>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PasswordSettingsForm {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct PasswordSettingsQuery {
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

pub async fn get_password_settings(
    CurrentUser(current_user): CurrentUser,
    Query(query): Query<PasswordSettingsQuery>,
) -> Response {
    let saved = query.updated.is_some_and(|updated| updated == 1);

    render_password_settings_page(
        PasswordSettingsTemplate::new(
            current_user.username,
            PasswordSettingsErrors::default(),
            saved,
        ),
        StatusCode::OK,
    )
}

pub async fn post_password_settings(
    State(state): State<AppState>,
    CurrentUser(current_user): CurrentUser,
    session: Session,
    Form(form): Form<PasswordSettingsForm>,
) -> Response {
    let mut errors = PasswordSettingsErrors::default();

    if form.current_password.is_empty() {
        errors.current_password = Some("Enter your current password.".to_owned());
    }

    if form.new_password.len() < PASSWORD_MIN_LENGTH {
        errors.new_password = Some(format!(
            "New password must be at least {PASSWORD_MIN_LENGTH} characters."
        ));
    } else if form.new_password == form.current_password {
        errors.new_password = Some("Choose a different password.".to_owned());
    }

    if !errors.is_empty() {
        return render_password_settings_page(
            PasswordSettingsTemplate::new(current_user.username, errors, false),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }

    let current_password_matches =
        match verify_password(&form.current_password, &current_user.password_hash) {
            Ok(is_valid) => is_valid,
            Err(hash_error) => {
                error!(
                    error = %hash_error,
                    user_id = current_user.id,
                    "failed to verify current password"
                );
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

    if !current_password_matches {
        return render_password_settings_page(
            PasswordSettingsTemplate::new(
                current_user.username,
                PasswordSettingsErrors {
                    current_password: Some("Current password is incorrect.".to_owned()),
                    ..PasswordSettingsErrors::default()
                },
                false,
            ),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }

    let new_password_hash = match hash_password(&form.new_password) {
        Ok(password_hash) => password_hash,
        Err(hash_error) => {
            error!(
                error = %hash_error,
                user_id = current_user.id,
                "failed to hash new password"
            );
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Err(db_error) = update_password_hash(&state, current_user.id, &new_password_hash).await {
        error!(
            error = %db_error,
            user_id = current_user.id,
            "failed to update password hash"
        );
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Err(session_error) = session.cycle_id().await {
        error!(
            error = %session_error,
            user_id = current_user.id,
            "failed to cycle session after password change"
        );
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Err(session_error) = session.insert("user_id", current_user.id).await {
        error!(
            error = %session_error,
            user_id = current_user.id,
            "failed to refresh session user after password change"
        );
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Redirect::to("/settings/password?updated=1").into_response()
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

async fn update_password_hash(
    state: &AppState,
    user_id: i64,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    query(
        r#"
        UPDATE users
        SET password_hash = $1, updated_at = NOW()
        WHERE id = $2
        "#,
    )
    .bind(password_hash)
    .bind(user_id)
    .execute(&state.db_pool)
    .await
    .map(|_| ())
}

fn render_password_settings_page(
    template: PasswordSettingsTemplate,
    status: StatusCode,
) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(
                error = %render_error,
                "failed to render password settings template"
            );
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
