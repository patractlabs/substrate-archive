use rdkafka::config::ClientConfig;
use rdkafka::message::OwnedHeaders;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct KafkaConfig {
	pub url: String,
	pub topic: String,
	pub key: String,
	mode: Option<String>,
	username: Option<String>,
	password: Option<String>,
}

#[derive(Clone)]
pub struct KafkaProducer {
	config: KafkaConfig,
	client: FutureProducer,
}

impl KafkaProducer {
	pub fn config(&self) -> &KafkaConfig {
		&self.config
	}
	pub fn new(c: &KafkaConfig) -> Self {
		//TODO remove expect
		let producer: FutureProducer = ClientConfig::new()
			.set("bootstrap.servers", c.url.as_str())
			.set("message.timeout.ms", "5000")
			.set("sasl.mechanisms", c.mode.as_ref().unwrap_or(&String::from("")).as_str())
			.set("sasl.username", c.username.as_ref().unwrap_or(&String::from("")).as_str())
			.set("sasl.password", c.password.as_ref().unwrap_or(&String::from("")).as_str())
			.create()
			.expect("producer creation error");
		log::info!("kafka producer {},{},{}", c.url, c.topic, c.mode.as_ref().unwrap_or(&String::from("")));
		KafkaProducer { config: c.clone(), client: producer.clone() }
	}

	pub async fn send(&self, key: &str, msg: &str) -> bool {
		let delivery_status = self
			.client
			.send(
				FutureRecord::to(self.config.topic.as_str())
					.payload(msg)
					.key(self.config.key.as_str())
					.headers(OwnedHeaders::new().add("header_key", "header_value")),
				Duration::from_secs(0),
			)
			.await;
		log::trace!("producer  send  {},  {},", key, msg);
		delivery_status
			.map(|(_, _)| true)
			.map_err(|e| log::error!("{:?} {:?} {:?},{:?}", self.config.url, self.config.topic, msg, e))
			.unwrap_or(false)
	}
}
