use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use regex::Regex;

pub struct Configs{
    pub kernel_config: HashMap<String, String>,
    pub patch_config:  HashMap<String, String>,
}

pub fn read_configs(kconfig: &str, config: &str) -> Configs
{
    Configs { 
        kernel_config: config_reader(kconfig),
        patch_config: config_reader(config) 
    }
}

pub fn config_reader(config_path: &str) -> HashMap<String, String> {
    let mut configs: HashMap<String, String> = HashMap::new();
    let file = File::open(config_path).expect("[x] Failed to open config file");
    let reader = BufReader::new(file);

    // regex to strip comments: "#" and everything after it
    let re_comment = Regex::new(r"#\s+").unwrap();

    for line in reader.lines() {
        match line {
            Ok(contents) => {
                // remove comments
                let no_comments = re_comment.replace_all(&contents, "").trim().to_string();

                if no_comments.is_empty() {
                    continue; // skip empty lines
                }

                // split by '=' first, fallback to whitespace
                let parts: Vec<&str> = if no_comments.contains('=') {
                    no_comments.splitn(2, '=').collect()
                } else {
                    no_comments.splitn(2, "is").collect()
                };

                if !parts.is_empty() {
                    let key = parts[0].trim().to_string();
                    let value = if parts.len() > 1 {
                        parts[1].trim().to_string()
                    } else {
                        String::new() // allow flags without value
                    };
                    configs.insert(key, value);
                }
            }
            Err(e) => eprintln!("[x] Error reading line: {}", e),
        }
    }

    configs
}
