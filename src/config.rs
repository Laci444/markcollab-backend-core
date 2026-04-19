use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_port")]
    pub port: u16,

    pub kafka_brokers: String,
    pub kafka_topic: String,
    #[serde(default = "default_message_timeout")]
    pub kafka_message_timeout: u16,
}

fn default_port() -> u16 {
    3000
}

fn default_message_timeout() -> u16 {
    5000
}

impl AppConfig {
    pub fn load() -> Result<Self, envy::Error> {
        let _ = dotenvy::dotenv();

        envy::from_env::<AppConfig>()
    }
}
