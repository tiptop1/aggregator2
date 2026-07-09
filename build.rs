use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
   pub parameters: HashMap<String, String>
}

const CONFIG_PARAM_FILE: &str = ".config-params.toml";
const CONFIG_FILE: &str = "config.toml";

fn main() {
    // Read config params not stored in code repository because of sensitive data
    let config_params_content = fs::read_to_string(CONFIG_PARAM_FILE).unwrap();
    let config_params: Config = toml::from_str(&config_params_content).unwrap();


    // In application configuration file, replace parameter placeholders with actual values from config params file
    let mut config_file_content = fs::read_to_string(CONFIG_FILE).unwrap();
    // let mut new_config_file_content = config_file_content.clone();
    for (k, v) in config_params.parameters.iter() {
        config_file_content = config_file_content.replace(&format!("${{{}}}", k), v);
    }

    // Write new configuration file conent
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let _ = fs::write(out_dir.join(CONFIG_FILE), config_file_content);

    println!("cargo:rerun-if-changed={}", CONFIG_FILE);
}