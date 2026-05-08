use askama::Template;
use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use sqlx::{query_as, query_scalar};
use tower_sessions::Session;
use tracing::error;

use crate::{
    models::User,
    password::hash_password,
    state::AppState,
    templates::{RegistrationErrors, RegistrationFormValues, RegistrationTemplate},
};

#[derive(Debug, Default, Deserialize)]
pub struct RegistrationForm {
    pub username: String,
    pub password: String,
    pub confirm: String,
}

pub async fn get_registration() -> Response {
    render_registration_page(
        RegistrationTemplate::new(RegistrationFormValues::default(), RegistrationErrors::default()),
        StatusCode::OK,
    )
}

pub async fn post_registration(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<RegistrationForm>,
) -> Response {
    let username = form.username.trim().to_owned();
    let form_values = RegistrationFormValues {
        username: username.clone(),
    };

    let mut errors = validate_registration_form(&form, &username);

    if errors.is_empty() {
        match username_exists(&state, &username).await {
            Ok(true) => {
                errors.username = Some("That username is already taken.".to_owned());
            }
            Ok(false) => {}
            Err(db_error) => {
                error!(error = %db_error, "failed to check username uniqueness");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    }

    if !errors.is_empty() {
        return render_registration_page(
            RegistrationTemplate::new(form_values, errors),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }

    let password_hash = match hash_password(&form.password) {
        Ok(password_hash) => password_hash,
        Err(hash_error) => {
            error!(error = %hash_error, "failed to hash password");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let created_user = match create_user(&state, &username, &password_hash).await {
        Ok(user) => user,
        Err(sqlx::Error::Database(db_error)) if db_error.code().as_deref() == Some("23505") => {
            return render_registration_page(
                RegistrationTemplate::new(
                    form_values,
                    RegistrationErrors {
                        username: Some("That username is already taken.".to_owned()),
                        ..RegistrationErrors::default()
                    },
                ),
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        Err(db_error) => {
            error!(error = %db_error, "failed to create user");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Err(session_error) = session.insert("user_id", created_user.id).await {
        error!(error = %session_error, "failed to write user id to session");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Redirect::to("/").into_response()
}

fn validate_registration_form(form: &RegistrationForm, username: &str) -> RegistrationErrors {
    let mut errors = RegistrationErrors::default();

    if username.len() < 3 {
        errors.username = Some("Username must be at least 3 characters.".to_owned());
    } else if username.len() > 32 {
        errors.username = Some("Username must be at most 32 characters.".to_owned());
    } else if !username
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        errors.username = Some("Use only letters, numbers, or underscores.".to_owned());
    }

    if form.password.len() < 8 {
        errors.password = Some("Password must be at least 8 characters.".to_owned());
    }

    if form.password != form.confirm {
        errors.confirm = Some("Password confirmation does not match.".to_owned());
    }

    errors
}

async fn username_exists(state: &AppState, username: &str) -> Result<bool, sqlx::Error> {
    query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
        .bind(username)
        .fetch_one(&state.db_pool)
        .await
}

async fn create_user(
    state: &AppState,
    username: &str,
    password_hash: &str,
) -> Result<User, sqlx::Error> {
    query_as::<_, User>(
        r#"
        INSERT INTO users (username, password_hash)
        VALUES ($1, $2)
        RETURNING id, username, password_hash, role, bio, created_at, updated_at
        "#,
    )
    .bind(username)
    .bind(password_hash)
    .fetch_one(&state.db_pool)
    .await
}

fn render_registration_page(template: RegistrationTemplate, status: StatusCode) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(error = %render_error, "failed to render registration template");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
