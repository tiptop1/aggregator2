use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub parameters: HashMap<String, String>,
}

const CONFIG_PARAM_FILE: &str = ".config-params.toml";
const CONFIG_FILE: &str = "config.toml";

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Read config params not stored in code repository because of sensitive data
    let config_params_content = fs::read_to_string(manifest_dir.join(CONFIG_PARAM_FILE)).unwrap();
    let config_params: Config = toml::from_str(&config_params_content).unwrap();

    // In application configuration file, replace parameter placeholders with actual values from config params file
    let config_file = manifest_dir.join(CONFIG_FILE);
    let mut config_file_content = fs::read_to_string(&config_file).unwrap();
    // let mut new_config_file_content = config_file_content.clone();
    for (k, v) in config_params.parameters.iter() {
        config_file_content = config_file_content.replace(&format!("${{{}}}", k), v);
    }

    // Write new configuration file conent
    let out_dir = manifest_dir.join("target").join("generated");
    fs::create_dir_all(&out_dir).expect("can't create directories structre");
    let eval_config_file = out_dir.join(CONFIG_FILE);

    let _ = fs::write(&eval_config_file, config_file_content);

    println!("cargo:rerun-if-changed={}", config_file.to_str().unwrap());
}
