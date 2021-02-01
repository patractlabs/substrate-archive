use std::{collections::HashMap, time::Duration};

use codec::Encode;
use rdkafka::{
	config::ClientConfig,
	producer::{FutureProducer, FutureRecord},
};
use serde::{Deserialize, Serialize};
use xtra::prelude::*;

use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
use sp_storage::{StorageData, StorageKey};

use crate::{error::Result, types::Metadata};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct KafkaConfig {
	/// topic names
	pub topic: KafkaTopicConfig,
	/// rdkafka config
	pub rdkafka: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct KafkaTopicConfig {
	/// topic metadata name
	pub metadata: String,
	/// topic block name
	pub block: String,
}

#[derive(Clone)]
pub struct KafkaProducer {
	config: KafkaConfig,
	producer: FutureProducer,
}

impl KafkaProducer {
	pub fn new(config: KafkaConfig) -> Self {
		assert!(Self::check_kafka_config(&config), "Invalid kafka configuration");

		let mut client = ClientConfig::new();
		for (k, v) in &config.rdkafka {
			client.set(k, v);
		}
		let producer = client.create::<FutureProducer>().expect("Producer creation error");
		log::info!("Kafka configuration: {:?}", config);
		Self { config, producer }
	}

	fn check_kafka_config(config: &KafkaConfig) -> bool {
		(config.rdkafka.get("metadata.broker.list").is_some() || config.rdkafka.get("bootstrap.servers").is_some())
			&& !config.topic.metadata.is_empty()
			&& !config.topic.block.is_empty()
	}

	pub async fn send(&self, topic: &str, key: &str, msg: &str) -> Result<()> {
		let record = FutureRecord::to(topic).key(key).payload(msg);
		let queue_timeout = Duration::from_secs(0);
		let delivery_status = self.producer.send(record, queue_timeout).await;
		match delivery_status {
			Ok(result) => {
				log::debug!("topic: {}, partition: {}, offset: {}", topic, result.0, result.1);
				Ok(())
			}
			Err(err) => {
				log::error!("topic: {}, error: {}, msg: {:?}", topic, err.0, err.1);
				Err(err.0.into())
			}
		}
	}

	pub fn config(&self) -> &KafkaConfig {
		&self.config
	}
}

#[derive(Clone, Serialize)]
pub struct KafkaMetadataValue<B: BlockT> {
	pub version: u32,
	meta: String,
	block_num: <<B as BlockT>::Header as HeaderT>::Number,
	hash: B::Hash,
}

impl<B: BlockT> KafkaMetadataValue<B> {
	pub fn from(meta: Metadata, block_num: <<B as BlockT>::Header as HeaderT>::Number, hash: B::Hash) -> Self {
		Self { version: meta.version(), meta: format!("0x{}", hex::encode(meta.meta())), block_num, hash }
	}
}

impl<B: BlockT> Message for KafkaMetadataValue<B> {
	type Result = ();
}

#[derive(Clone, Serialize)]
pub struct KafkaBlockValue<B: BlockT> {
	parent_hash: <B as BlockT>::Hash,
	hash: <B as BlockT>::Hash,
	pub block_num: <<B as BlockT>::Header as HeaderT>::Number,
	state_root: <B as BlockT>::Hash,
	extrinsics_root: <B as BlockT>::Hash,
	digest: String,
	extrinsics: Vec<String>,
	spec: u32,
	is_full: bool,
	pub changes: Vec<(StorageKey, Option<StorageData>)>,
}

impl<B: BlockT> KafkaBlockValue<B> {
	pub fn from(block: B, spec: u32, is_full: bool, changes: Vec<(StorageKey, Option<StorageData>)>) -> Self {
		let extrinsics =
			block.extrinsics().iter().map(|ext| format!("0x{}", hex::encode(ext.encode()))).collect::<Vec<_>>();
		Self {
			parent_hash: *block.header().parent_hash(),
			hash: block.hash(),
			block_num: *block.header().number(),
			state_root: *block.header().state_root(),
			extrinsics_root: *block.header().extrinsics_root(),
			digest: format!("0x{}", hex::encode(block.header().digest().encode())),
			extrinsics,
			spec,
			is_full,
			changes,
		}
	}
}

impl<B: BlockT> Message for KafkaBlockValue<B> {
	type Result = ();
}
