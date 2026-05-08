use axum::{
    extract::{FromRef, FromRequestParts, Request, State},
    http::StatusCode,
    http::request::Parts,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use sqlx::query_as;
use std::{error::Error, fmt};
use tower_sessions::Session;
use tracing::error;

use crate::{
    models::{User, UserRole},
    state::AppState,
};

const USER_ID_SESSION_KEY: &str = "user_id";

#[derive(Clone, Debug)]
pub struct CurrentUser(pub User);

#[derive(Clone, Debug, Default)]
pub struct MaybeCurrentUser(pub Option<User>);

impl MaybeCurrentUser {
    pub fn is_authenticated(&self) -> bool {
        self.0.is_some()
    }
}

impl<S> FromRequestParts<S> for MaybeCurrentUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(current_user) = parts.extensions.get::<CurrentUser>() {
            return Ok(Self(Some(current_user.0.clone())));
        }

        let app_state = AppState::from_ref(state);
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;

        let current_user = load_current_user(&app_state, &session)
            .await
            .map_err(internal_server_error)?;

        if let Some(user) = current_user.as_ref() {
            parts.extensions.insert(CurrentUser(user.clone()));
        }

        Ok(Self(current_user))
    }
}

impl<S> FromRequestParts<S> for CurrentUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let maybe_current_user = MaybeCurrentUser::from_request_parts(parts, state).await?;

        match maybe_current_user.0 {
            Some(user) => Ok(Self(user)),
            None => Err(Redirect::to("/login").into_response()),
        }
    }
}

pub async fn require_auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let (mut parts, body) = request.into_parts();
    let session = match Session::from_request_parts(&mut parts, &state).await {
        Ok(session) => session,
        Err(rejection) => return rejection.into_response(),
    };

    let current_user = match load_current_user(&state, &session).await {
        Ok(Some(user)) => user,
        Ok(None) => return Redirect::to("/login").into_response(),
        Err(db_error) => return internal_server_error(db_error),
    };

    parts.extensions.insert(CurrentUser(current_user));

    next.run(Request::from_parts(parts, body)).await
}

pub async fn require_admin(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let (mut parts, body) = request.into_parts();

    if let Some(current_user) = parts.extensions.get::<CurrentUser>() {
        return if current_user.0.role == UserRole::Admin {
            next.run(Request::from_parts(parts, body)).await
        } else {
            StatusCode::FORBIDDEN.into_response()
        };
    }

    let session = match Session::from_request_parts(&mut parts, &state).await {
        Ok(session) => session,
        Err(rejection) => return rejection.into_response(),
    };

    let current_user = match load_current_user(&state, &session).await {
        Ok(Some(user)) if user.role == UserRole::Admin => user,
        Ok(Some(_)) | Ok(None) => return StatusCode::FORBIDDEN.into_response(),
        Err(db_error) => return internal_server_error(db_error),
    };

    parts.extensions.insert(CurrentUser(current_user));

    next.run(Request::from_parts(parts, body)).await
}

async fn load_current_user(
    state: &AppState,
    session: &Session,
) -> Result<Option<User>, AuthLoadError> {
    let user_id = session
        .get::<i64>(USER_ID_SESSION_KEY)
        .await
        .map_err(AuthLoadError::Session)?;

    let Some(user_id) = user_id else {
        return Ok(None);
    };

    query_as::<_, User>(
        r#"
        SELECT id, username, password_hash, role, bio, created_at, updated_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(AuthLoadError::Database)
}

fn internal_server_error(error: impl std::fmt::Display) -> Response {
    error!(error = %error, "authentication flow failed");
    axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

#[derive(Debug)]
enum AuthLoadError {
    Session(tower_sessions::session::Error),
    Database(sqlx::Error),
}

impl fmt::Display for AuthLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session(error) => write!(f, "session read failed: {error}"),
            Self::Database(error) => write!(f, "user lookup failed: {error}"),
        }
    }
}

impl Error for AuthLoadError {}
