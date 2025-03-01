mod kafka_connector;

use kafka_connector::{
    consumer::create_consumer,
    publisher::{create_publisher, send_message},
};
use rdkafka::Message;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    main,
};
use uuid::Uuid;

#[main]
async fn main() {
    let server = "192.168.0.10:9092";
    let topic = "test-topic";

    let publisher = create_publisher(server).expect("Failed to create publisher");
    let consumer = create_consumer(server, &Uuid::new_v4().to_string(), &[topic])
        .expect("Failed to create consumer");

    let mut stdout = tokio::io::stdout();
    let mut input_lines = BufReader::new(tokio::io::stdin()).lines();

    loop {
        stdout.write_all(b"> ").await.unwrap();
        stdout.flush().await.unwrap();

        tokio::select! {
            message = consumer.recv() => {
                let message = message.expect("Failed to read message");
                let payload = message.payload().unwrap();
                stdout.write_all(payload).await.unwrap();
                stdout.write_all(b"\n").await.unwrap();
            }
            line = input_lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        send_message(&publisher, topic, line).await.expect("Failed to send message");
                    }
                    _ => break,
                }
            }
        }
    }
}
