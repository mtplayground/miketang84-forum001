use axum::{
    extract::State,
    http::StatusCode,
    middleware,
    routing::{get, post},
    Router,
};
use sqlx::query_scalar;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_sessions::{cookie::SameSite, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;
use tracing::{error, info};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod auth;
mod categories;
mod config;
mod login;
mod markdown;
mod models;
mod pagination;
mod password;
mod posts;
mod profile;
mod registration;
mod session;
mod settings;
mod state;
mod templates;
mod threads;

use config::Config;
use session::session_encryption_key;
use state::AppState;

use std::error::Error;

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    let config = Config::from_env()?;
    init_tracing(&config.rust_log)?;
    let app_state = AppState::from_config(&config).await?;
    let session_store = PostgresStore::new(app_state.db_pool.clone());
    let session_layer = SessionManagerLayer::new(session_store)
        .with_same_site(SameSite::Lax)
        .with_secure(!cfg!(debug_assertions))
        .with_private(session_encryption_key(&config.session_secret)?);
    let require_auth_layer = middleware::from_fn_with_state(app_state.clone(), auth::require_auth);
    let require_auth_layer_for_settings =
        middleware::from_fn_with_state(app_state.clone(), auth::require_auth);
    let require_auth_layer_for_password =
        middleware::from_fn_with_state(app_state.clone(), auth::require_auth);
    let require_auth_layer_for_thread_create =
        middleware::from_fn_with_state(app_state.clone(), auth::require_auth);

    let app = Router::new()
        .route("/", get(categories::list_categories))
        .route("/login", get(login::get_login).post(login::post_login))
        .route(
            "/logout",
            post(login::post_logout).route_layer(require_auth_layer),
        )
        .route(
            "/register",
            get(registration::get_registration).post(registration::post_registration),
        )
        .route(
            "/settings/profile",
            get(settings::get_profile_settings)
                .post(settings::post_profile_settings)
                .route_layer(require_auth_layer_for_settings),
        )
        .route(
            "/settings/password",
            get(settings::get_password_settings)
                .post(settings::post_password_settings)
                .route_layer(require_auth_layer_for_password),
        )
        .route(
            "/c/{slug}/new",
            get(threads::get_create_thread)
                .post(threads::post_create_thread)
                .route_layer(require_auth_layer_for_thread_create),
        )
        .route(
            "/t/{thread_ref}",
            get(threads::show_thread).post(threads::post_reply_to_thread),
        )
        .route(
            "/c/{category_slug}/t/{thread_slug}",
            get(threads::show_thread_legacy),
        )
        .route("/c/{slug}", get(categories::show_category))
        .route("/u/{username}", get(profile::show_profile))
        .route("/healthz", get(healthz))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(app_state)
        .layer(session_layer)
        .layer(ServiceBuilder::new());

    let listener = TcpListener::bind(config.bind_addr).await?;
    info!(
        bind_addr = %config.bind_addr,
        database_configured = !config.database_url.is_empty(),
        session_secret_configured = !config.session_secret.is_empty(),
        "starting server"
    );

    axum::serve(listener, app).await?;

    Ok(())
}

fn init_tracing(rust_log: &str) -> AppResult<()> {
    let env_filter = EnvFilter::try_new(rust_log)?;

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_target(true),
        )
        .try_init()?;

    Ok(())
}

async fn healthz(State(state): State<AppState>) -> Result<&'static str, StatusCode> {
    query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db_pool)
        .await
        .map(|_| "ok")
        .map_err(|error| {
            error!(error = %error, "database health check failed");
            StatusCode::SERVICE_UNAVAILABLE
        })
}
