use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tracing::info;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;

use config::Config;

use std::error::Error;

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    let config = Config::from_env()?;
    init_tracing(&config.rust_log)?;

    let app = Router::new()
        .route("/", get(root))
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

async fn root() -> &'static str {
    "Hello, world!"
}
