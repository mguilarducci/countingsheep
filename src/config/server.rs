//! Configuration for the HTTP server.

use std::net::IpAddr;

use anyhow::Context;

pub struct Server {
    pub ip: IpAddr,
    pub port: u16,
}

impl Server {
    /// Builds the server configuration from environment variables.
    ///
    /// Binds to all interfaces (`0.0.0.0`) when running under Heroku or Docker,
    /// and to localhost otherwise. The port is read from `PORT`, defaulting to
    /// `8888`.
    pub fn from_environment() -> anyhow::Result<Self> {
        let docker = std::env::var("DEV_DOCKER").is_ok();
        let heroku = std::env::var("HEROKU").is_ok();

        let ip = if heroku || docker {
            [0, 0, 0, 0].into()
        } else {
            [127, 0, 0, 1].into()
        };

        let port = match std::env::var("PORT") {
            Ok(value) => value.parse().context("PORT must be a valid port number")?,
            Err(_) => 8888,
        };

        Ok(Server { ip, port })
    }
}
