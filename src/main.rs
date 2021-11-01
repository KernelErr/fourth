mod config;
mod plugins;
mod servers;

use crate::config::Config;
use crate::servers::Server;

use std::env;
use log::{debug, error};

fn main() {
    let config_path = env::var("FOURTH_CONFIG").unwrap_or_else(|_| "/etc/fourth/config.yaml".to_string());

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

    let res = server.run();
    error!("Server returned an error: {:?}", res);
}
