pub struct RockConfig {
    pub root: String,
    pub host: String,
    pub port: u16,
}

impl RockConfig {
    pub fn new(root: String, host: String, port: u16) -> RockConfig {
        RockConfig {
            root: root,
            host: host,
            port: port,
        }
    }
}