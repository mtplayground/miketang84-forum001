use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::Html,
    routing::get,
    Router,
};
use sqlx::query_scalar;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tracing::{error, info};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod state;
mod templates;

use config::Config;
use state::AppState;
use templates::HomeTemplate;

use std::error::Error;

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    let config = Config::from_env()?;
    init_tracing(&config.rust_log)?;
    let app_state = AppState::from_config(&config).await?;

    let app = Router::new()
        .route("/", get(root))
        .route("/healthz", get(healthz))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(app_state)
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

async fn root() -> Result<Html<String>, StatusCode> {
    HomeTemplate {
        page_title: "Home",
        heading: "Forum foundation is online.",
        intro: "This starter page is rendered with Askama and inherits the shared base layout that future forum pages will extend.",
    }
    .render()
    .map(Html)
    .map_err(|error| {
        error!(error = %error, "failed to render home template");
        StatusCode::INTERNAL_SERVER_ERROR
    })
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
