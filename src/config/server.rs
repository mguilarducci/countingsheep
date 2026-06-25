//! Configuration for the HTTP server.

use std::net::IpAddr;

use countingsheep_env_vars::{var, var_parsed};

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
        let docker = var("DEV_DOCKER")?.is_some();
        let heroku = var("HEROKU")?.is_some();

        let ip = if heroku || docker {
            [0, 0, 0, 0].into()
        } else {
            [127, 0, 0, 1].into()
        };

        let port = var_parsed::<u16>("PORT")?.unwrap_or(8888);

        Ok(Server { ip, port })
    }
}
