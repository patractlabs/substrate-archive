use std::marker::PhantomData;

use xtra::prelude::*;

use sp_runtime::traits::{Block as BlockT, NumberFor};

use crate::{
	error::Result,
	kafka::{KafkaBlockValue, KafkaConfig, KafkaMetadataValue, KafkaProducer},
	types::Die,
};

pub struct KafkaActor<B: BlockT> {
	producer: KafkaProducer,
	_marker: PhantomData<B>,
}

impl<B: BlockT> KafkaActor<B> {
	pub fn new(config: KafkaConfig) -> Self {
		Self { producer: KafkaProducer::new(config), _marker: PhantomData }
	}

	async fn metadata_handler(&self, meta: KafkaMetadataValue<B>) -> Result<()> {
		let topic = &self.producer.config().topic.metadata;
		let key = meta.version.to_string();
		let msg = serde_json::to_string(&meta)?;
		log::debug!("Kafka publish metadata, version = {}", meta.version);
		self.producer.send(topic, &key, &msg).await?;
		Ok(())
	}

	async fn block_handler(&self, blk: KafkaBlockValue<B>) -> Result<()>
	where
		NumberFor<B>: Into<u32>,
	{
		let topic = &self.producer.config().topic.block;
		let key = blk.block_num.to_string();
		let msg = serde_json::to_string(&blk)?;
		log::debug!("Kafka publish block, number = {}, hash = {}", blk.block_num, blk.hash);
		self.producer.send(topic, &key, &msg).await?;
		Ok(())
	}
}

impl<B: BlockT> Actor for KafkaActor<B> {}

#[async_trait::async_trait]
impl<B: BlockT> Handler<KafkaMetadataValue<B>> for KafkaActor<B> {
	async fn handle(&mut self, meta: KafkaMetadataValue<B>, _ctx: &mut Context<Self>) {
		if let Err(e) = self.metadata_handler(meta).await {
			log::error!("{}", e.to_string());
		}
	}
}

#[async_trait::async_trait]
impl<B> Handler<KafkaBlockValue<B>> for KafkaActor<B>
where
	B: BlockT,
	NumberFor<B>: Into<u32>,
{
	async fn handle(&mut self, blk: KafkaBlockValue<B>, _: &mut Context<Self>) {
		if let Err(e) = self.block_handler(blk).await {
			log::error!("{}", e.to_string())
		}
	}
}

#[async_trait::async_trait]
impl<B: BlockT + Unpin> Handler<Die> for KafkaActor<B>
where
	NumberFor<B>: Into<u32>,
	B::Hash: Unpin,
{
	async fn handle(&mut self, _: Die, ctx: &mut Context<Self>) {
		ctx.stop();
	}
}
