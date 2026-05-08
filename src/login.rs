use askama::Template;
use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use sqlx::query_as;
use tower_sessions::Session;
use tracing::error;

use crate::{
    models::User,
    password::verify_password,
    state::AppState,
    templates::{LoginErrors, LoginFormValues, LoginTemplate},
};

#[derive(Debug, Default, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn get_login(session: Session) -> Response {
    let is_authenticated = match session.get::<i64>("user_id").await {
        Ok(user_id) => user_id.is_some(),
        Err(session_error) => {
            error!(error = %session_error, "failed to read auth state for login page");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    render_login_page(
        LoginTemplate::new(LoginFormValues::default(), LoginErrors::default(), is_authenticated),
        StatusCode::OK,
    )
}

pub async fn post_login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Response {
    let username = form.username.trim().to_owned();
    let form_values = LoginFormValues {
        username: username.clone(),
    };
    let is_authenticated = match session.get::<i64>("user_id").await {
        Ok(user_id) => user_id.is_some(),
        Err(session_error) => {
            error!(error = %session_error, "failed to read auth state during login");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut errors = LoginErrors::default();

    if username.is_empty() {
        errors.username = Some("Enter your username.".to_owned());
    }

    if form.password.is_empty() {
        errors.password = Some("Enter your password.".to_owned());
    }

    if !errors.is_empty() {
        return render_login_page(
            LoginTemplate::new(form_values, errors, is_authenticated),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }

    let user = match find_user_by_username(&state, &username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return render_login_page(
                LoginTemplate::new(
                    form_values,
                    LoginErrors {
                        general: Some("Invalid username or password.".to_owned()),
                        ..LoginErrors::default()
                    },
                    is_authenticated,
                ),
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        Err(db_error) => {
            error!(error = %db_error, "failed to fetch user for login");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let is_valid_password = match verify_password(&form.password, &user.password_hash) {
        Ok(result) => result,
        Err(hash_error) => {
            error!(error = %hash_error, "failed to verify password");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if !is_valid_password {
        return render_login_page(
            LoginTemplate::new(
                form_values,
                LoginErrors {
                    general: Some("Invalid username or password.".to_owned()),
                    ..LoginErrors::default()
                },
                is_authenticated,
            ),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }

    if let Err(session_error) = session.cycle_id().await {
        error!(error = %session_error, "failed to cycle session id during login");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Err(session_error) = session.insert("user_id", user.id).await {
        error!(error = %session_error, "failed to store user id during login");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Redirect::to("/").into_response()
}

pub async fn post_logout(session: Session) -> Response {
    if let Err(session_error) = session.flush().await {
        error!(error = %session_error, "failed to flush session during logout");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Redirect::to("/").into_response()
}

async fn find_user_by_username(state: &AppState, username: &str) -> Result<Option<User>, sqlx::Error> {
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

fn render_login_page(template: LoginTemplate, status: StatusCode) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(render_error) => {
            error!(error = %render_error, "failed to render login template");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
