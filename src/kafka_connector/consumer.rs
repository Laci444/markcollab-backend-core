use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    error::KafkaError,
    ClientConfig,
};

pub fn create_consumer(
    bootstrap_servers: &str,
    group_id: &str,
    topics: &[&str],
) -> Result<StreamConsumer, KafkaError> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", bootstrap_servers)
        .set("group.id", group_id)
        .create()?;
    consumer.subscribe(topics)?;
    Ok(consumer)
}
