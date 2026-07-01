use std::net::SocketAddr;
use std::sync::Arc;

use countingsheep::config::Server;
use countingsheep::{App, build_handler};
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};

const CORE_THREADS: usize = 4;

fn main() -> anyhow::Result<()> {
    // Bind exposure is a deployment signal and must never come from a stray
    // `.env` file, so read it from the real process environment BEFORE `.env`
    // is loaded below. `var_os` treats any value (including non-UTF-8) as
    // "set", matching the flag semantics.
    let expose_externally =
        std::env::var_os("HEROKU").is_some() || std::env::var_os("DEV_DOCKER").is_some();

    // Load `.env` once, up front, so `RUST_LOG` (read by the tracing
    // `EnvFilter`) and `PORT` see it. Real env vars are never overridden.
    countingsheep_env_vars::load();

    countingsheep::util::tracing::init();

    let config = Server::from_environment(expose_externally)?;

    // Kafka is required: an unconfigured broker fails startup here (D6).
    let producer: std::sync::Arc<dyn countingsheep::Producer> =
        std::sync::Arc::new(countingsheep::KafkaProducer::from_config(&config.kafka)?);

    let app = App::builder()
        .config(Arc::new(config))
        .producer(producer.clone())
        .build();
    let app = Arc::new(app);

    let handler = build_handler(app.clone());

    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    builder.worker_threads(CORE_THREADS);
    let rt = builder.build()?;

    let make_service = handler.into_make_service_with_connect_info::<SocketAddr>();

    // Block the main thread until the server has shut down.
    rt.block_on(async {
        // Initialize error tracking inside the runtime (the PostHog client is
        // built asynchronously). Safe by default: a no-op when unconfigured.
        countingsheep::observability::error_tracking::init(&app.config.posthog).await;

        let listener = TcpListener::bind((app.config.ip, app.config.port)).await?;

        let addr = listener.local_addr()?;
        println!("Listening at http://{addr}");

        // Run the server with graceful shutdown.
        let result = axum::serve(listener, make_service)
            .with_graceful_shutdown(shutdown_signal())
            .await;

        // Drain any buffered exception events before the process exits.
        countingsheep::observability::error_tracking::shutdown().await;

        // Drain librdkafka's buffer before exit (mirrors error_tracking::shutdown).
        producer.flush(std::time::Duration::from_secs(30));

        result
    })?;

    println!("Server has gracefully shut down!");
    Ok(())
}

async fn shutdown_signal() {
    let interrupt = async {
        signal(SignalKind::interrupt())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    let terminate = async {
        signal(SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = interrupt => {},
        _ = terminate => {},
    }
}
