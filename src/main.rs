use std::{env, error::Error, net::SocketAddr};

use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tracing::info;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    dotenvy::dotenv().ok();
    init_tracing()?;

    let bind_addr = load_bind_addr()?;
    let app = Router::new()
        .route("/", get(root))
        .layer(ServiceBuilder::new());

    let listener = TcpListener::bind(bind_addr).await?;
    info!(%bind_addr, "starting server");

    axum::serve(listener, app).await?;

    Ok(())
}

fn init_tracing() -> AppResult<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))?;

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

fn load_bind_addr() -> AppResult<SocketAddr> {
    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned());

    bind_addr.parse::<SocketAddr>().map_err(|error| {
        format!("failed to parse BIND_ADDR `{bind_addr}` as socket address: {error}").into()
    })
}

async fn root() -> &'static str {
    "Hello, world!"
}
