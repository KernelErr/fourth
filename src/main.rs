mod config;
mod plugins;
mod servers;

use crate::config::Config;
use crate::servers::Server;

use log::{debug, error};
use std::env;

fn main() {
    let config_path =
        env::var("FOURTH_CONFIG").unwrap_or_else(|_| "/etc/fourth/config.yaml".to_string());

    let config = match Config::new(&config_path) {
        Ok(config) => config,
        Err(e) => {
            println!("Could not load config: {:?}", e);
            std::process::exit(1);
        }
    };
    debug!("{:?}", config);

    let mut server = Server::new(config.base);
    debug!("{:?}", server);

    let _ = server.run();
    error!("Server ended with errors");
}
