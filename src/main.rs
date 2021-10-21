mod config;
mod servers;

use crate::config::Config;
use crate::servers::Server;

use log::{debug, error};

fn main() {
    let config = match Config::new("/etc/fourth/config.yaml") {
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
