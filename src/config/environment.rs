use dotenv::dotenv;
use lazy_static::lazy_static;
use std::env;

use crate::dto::environment::Environment;


// get data from environment manual
pub fn get_server_url() -> Result<String, String> {
    dotenv::dotenv().ok();
    let server_host = env::var("SERVER_HOST").expect("SERVER_HOST is not set");
    let server_port = env::var("SERVER_PORT").expect("SERVER_PORT is not set");
    Ok(format!("{}:{}", server_host, server_port))
}

// get data from environment using dotenv, then serialize to Config model
pub fn get_config() -> Environment {
    dotenv().ok();

    match envy::from_env::<Environment>() {
        Ok(config) => config,
        Err(error) => panic!("Environment Error: {:#?}", error),
    }
}

// get data from json
pub fn get_config_from_json() -> Environment {
    let config_file: &'static str = "env.json";
    let config = Environment::from_file(config_file);
    config
}

// save Config model to heap (avoid repeated serialization)
lazy_static! {
    pub static ref CONFIG: Environment = get_config();
}
