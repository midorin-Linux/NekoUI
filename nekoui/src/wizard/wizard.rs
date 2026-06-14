use anyhow::Result;

use super::{Config, ServerConfig};

pub fn run_wizard() -> Result<Config> {
    println!("The wizard is currently being prepared. Default values will be saved.");

    let config = Config {
        server: ServerConfig::default(),
    };

    Ok(config)
}
