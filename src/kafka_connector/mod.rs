use rdkafka::{
    admin::{
        AdminClient, AdminOptions, AlterConfig, NewTopic, ResourceSpecifier, TopicReplication,
    },
    client::DefaultClientContext,
    consumer::{Consumer, StreamConsumer},
    error::KafkaError,
    producer::{future_producer::OwnedDeliveryResult, FutureProducer, FutureRecord},
    util::Timeout,
    ClientConfig,
};
use uuid::Uuid;

pub struct KafkaConnector {
    bootstrap_servers: String,
    admin_client: AdminClient<DefaultClientContext>,
    publisher: FutureProducer,
    number_of_partitions: i32,
    replication_factor: i32,
    retention_ms: i64,
}

impl KafkaConnector {
    fn get_basic_config(bootstrap_servers: &str) -> ClientConfig {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", bootstrap_servers);
        config
    }

    pub fn new(
        bootstrap_servers: &str,
        number_of_partitions_per_topic: i32,
        replication_factor: i32,
        retention_ms: i64,
    ) -> Result<Self, KafkaError> {
        Ok(Self {
            bootstrap_servers: bootstrap_servers.to_string(),
            admin_client: KafkaConnector::get_basic_config(bootstrap_servers).create()?,
            publisher: KafkaConnector::get_basic_config(bootstrap_servers).create()?,
            number_of_partitions: number_of_partitions_per_topic,
            replication_factor,
            retention_ms,
        })
    }

    pub fn create_consumer(&self, topics: &[&str]) -> Result<StreamConsumer, KafkaError> {
        let consumer: StreamConsumer = KafkaConnector::get_basic_config(&self.bootstrap_servers)
            .set("group.id", Uuid::new_v4())
            .create()?;
        consumer.subscribe(topics)?;
        Ok(consumer)
    }

    pub async fn send_message(&self, topic: &str, message: String) -> OwnedDeliveryResult {
        let record: FutureRecord<(), String> = FutureRecord::to(topic).payload(&message);
        self.publisher.send(record, Timeout::Never).await
    }

    pub async fn create_topic(&self, topic_name: &str) -> Result<(), KafkaError> {
        let new_topic = NewTopic::new(
            topic_name,
            self.number_of_partitions,
            TopicReplication::Fixed(self.replication_factor),
        );

        self.admin_client
            .create_topics(&[new_topic], &AdminOptions::new())
            .await?;

        let retention_converted = &self.retention_ms.to_string();

        let alter_config = AlterConfig::new(ResourceSpecifier::Topic(topic_name))
            .set("retention.ms", retention_converted);

        self.admin_client
            .alter_configs(&[alter_config], &AdminOptions::new())
            .await?;

        Ok(())
    }

    pub async fn delete_topic(&self, topic_name: &str) -> Result<(), KafkaError> {
        self.admin_client
            .delete_topics(&[topic_name], &AdminOptions::new())
            .await?;

        Ok(())
    }
}
