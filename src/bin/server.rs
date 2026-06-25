use std::net::SocketAddr;
use std::sync::Arc;

use countingsheep::config::Server;
use countingsheep::{App, build_handler};
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};

const CORE_THREADS: usize = 4;

fn main() -> anyhow::Result<()> {
    countingsheep::util::tracing::init();

    let config = Server::from_environment()?;

    let app = App::builder().config(Arc::new(config)).build();
    let app = Arc::new(app);

    let handler = build_handler(app.clone());

    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    builder.worker_threads(CORE_THREADS);
    let rt = builder.build()?;

    let make_service = handler.into_make_service_with_connect_info::<SocketAddr>();

    // Block the main thread until the server has shut down.
    rt.block_on(async {
        let listener = TcpListener::bind((app.config.ip, app.config.port)).await?;

        let addr = listener.local_addr()?;
        println!("Listening at http://{addr}");

        // Run the server with graceful shutdown.
        axum::serve(listener, make_service)
            .with_graceful_shutdown(shutdown_signal())
            .await
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
