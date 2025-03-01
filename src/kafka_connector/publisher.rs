use rdkafka::{
    error::KafkaError,
    producer::{future_producer::OwnedDeliveryResult, FutureProducer, FutureRecord},
    util::Timeout,
    ClientConfig,
};

pub fn create_publisher(bootstrap_servers: &str) -> Result<FutureProducer, KafkaError> {
    ClientConfig::new()
        .set("bootstrap.servers", bootstrap_servers)
        .create()
}

pub async fn send_message(
    publisher: &FutureProducer,
    topic: &str,
    message: String,
) -> OwnedDeliveryResult {
    let record: FutureRecord<(), String> = FutureRecord::to(topic).payload(&message);
    publisher.send(record, Timeout::Never).await
}
