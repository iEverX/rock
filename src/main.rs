mod rock;
mod config;
use std::str::FromStr;

fn main() {
    let root = String::from_str("E:\\project\\rust\\static_rock").unwrap();
    let host = String::from_str("127.0.0.1").unwrap();
    let c = config::RockConfig::new(root, host, 9999);
    let server = rock::Rock::new(c);
    server.start();
}
