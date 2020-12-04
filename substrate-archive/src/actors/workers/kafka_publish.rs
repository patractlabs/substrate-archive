use crate::actors::msg::VecStorageWrap;
use crate::database::StorageModel;
use crate::kafka::{KafkaConfig, KafkaProducer};
use serde_json::json;
use serde_json::value::Value as JsonValue;
use sp_runtime::traits::{Block as BlockT, NumberFor};
use std::marker::PhantomData;
use substrate_archive_common::{types::Storage, Result};
use xtra::prelude::*;

#[derive(Clone)]
pub struct KafkaPublishActor<B: BlockT> {
	producers: Vec<KafkaProducer>,
	_marker: PhantomData<B>,
}

impl<B: BlockT> KafkaPublishActor<B> {
	pub async fn new(configs: &Vec<KafkaConfig>) -> Result<Self> {
		let mut producers = Vec::new();
		for c in configs.iter() {
			producers.push(KafkaProducer::new(c));
		}

		log::info!("KafkaPublishActor init {}  producers", producers.len());
		Ok(Self { producers: producers, _marker: PhantomData })
	}
	async fn send_all_produces(&self, messages: &Vec<JsonValue>) {
		if messages.len() > 0 {
			for p in self.producers.iter() {
				p.send(p.config().key.as_str(), json!({ "data": messages }).to_string().as_str()).await;
			}
		}
	}
	async fn batch_storage_handler(&self, storage: Vec<Storage<B>>) -> Result<()> {
		let now = std::time::Instant::now();
		let storage = Vec::<StorageModel<B>>::from(VecStorageWrap(storage));
		let mut messages: Vec<JsonValue> = vec![];

		for record in storage.iter() {
			let m = json!({
				"block_num":record.block_num(),
				"hash":record.hash(),
				"is_full":record.is_full(),
				"key":hex::encode(record.key().0.as_slice()),
				"storage":hex::encode( record.data().map(|d| d.0.as_slice()).unwrap_or(&[]))
			});
			messages.push(m);
			if messages.len() >= 10 {
				self.send_all_produces(&messages).await;
				messages.clear();
			}
		}
		self.send_all_produces(&messages).await;

		log::debug!("Took {:?} to publish {:#?} messages", now.elapsed(), storage.len());
		Ok(())
	}
}

impl<B: BlockT> Actor for KafkaPublishActor<B> {}

#[async_trait::async_trait]
impl<B: BlockT> Handler<VecStorageWrap<B>> for KafkaPublishActor<B> {
	async fn handle(&mut self, storage: VecStorageWrap<B>, _ctx: &mut Context<Self>) {
		if let Err(e) = self.batch_storage_handler(storage.0).await {
			log::error!("{}", e.to_string());
		}
	}
}

#[async_trait::async_trait]
impl<B: BlockT + Unpin> Handler<super::Die> for KafkaPublishActor<B>
where
	NumberFor<B>: Into<u32>,
	B::Hash: Unpin,
{
	async fn handle(&mut self, _: super::Die, ctx: &mut Context<Self>) -> Result<()> {
		ctx.stop();
		Ok(())
	}
}
