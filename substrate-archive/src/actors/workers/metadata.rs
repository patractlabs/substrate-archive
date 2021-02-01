// Copyright 2017-2021 Parity Technologies (UK) Ltd.
// This file is part of substrate-archive.

// substrate-archive is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
// substrate-archive is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with substrate-archive.  If not, see <http://www.gnu.org/licenses/>.

use itertools::Itertools;
use xtra::prelude::*;

#[cfg(feature = "kafka")]
use sp_runtime::traits::Header as HeaderT;
use sp_runtime::{
	generic::BlockId,
	traits::{Block as BlockT, NumberFor},
};
use substrate_archive_backend::Meta;

#[cfg(feature = "kafka")]
use crate::{
	actors::workers::KafkaActor,
	kafka::{KafkaBlockValue, KafkaMetadataValue},
};
use crate::{
	actors::{
		actor_pool::ActorPool,
		workers::database::{DatabaseActor, GetState},
	},
	database::{queries, DbConn},
	error::Result,
	types::{BatchBlock, Block, Die, Metadata},
};

/// Actor to fetch metadata about a block/blocks from RPC
/// Accepts workers to decode blocks and a URL for the RPC
pub struct MetadataActor<B: BlockT> {
	conn: DbConn,
	addr: Address<ActorPool<DatabaseActor<B>>>,
	meta: Meta<B>,
	#[cfg(feature = "kafka")]
	kafka: Option<Address<KafkaActor<B>>>,
}

#[cfg(not(feature = "kafka"))]
impl<B: BlockT + Unpin> MetadataActor<B> {
	pub async fn new(addr: Address<ActorPool<DatabaseActor<B>>>, meta: Meta<B>) -> Result<Self> {
		let conn = addr.send(GetState::Conn.into()).await?.await?.conn();
		Ok(Self { conn, addr, meta })
	}

	// checks if the metadata exists in the database
	// if it doesn't exist yet, fetch metadata and insert it
	async fn meta_checker(&mut self, ver: u32, hash: B::Hash) -> Result<()> {
		if !queries::check_if_meta_exists(ver, &mut self.conn).await? {
			let meta = self.meta.clone();
			log::info!("Getting metadata for hash {}, version {}", hex::encode(hash.as_ref()), ver);
			let meta = smol::unblock(move || meta.metadata(&BlockId::hash(hash))).await?;
			let meta: sp_core::Bytes = meta.into();
			let meta = Metadata::new(ver, meta.0);
			self.addr.send(meta.clone().into()).await?.await;
		}
		Ok(())
	}

	async fn block_handler(&mut self, blk: Block<B>) -> Result<()>
	where
		NumberFor<B>: Into<u32>,
	{
		let hash = blk.inner.block.hash();
		self.meta_checker(blk.spec, hash).await?;
		self.addr.send(blk.clone().into()).await?.await;
		Ok(())
	}

	async fn batch_block_handler(&mut self, blks: BatchBlock<B>) -> Result<()>
	where
		NumberFor<B>: Into<u32>,
	{
		for blk in blks.inner().iter().unique_by(|&blk| blk.spec) {
			self.meta_checker(blk.spec, blk.inner.block.hash()).await?;
		}
		self.addr.send(blks.clone().into()).await?.await;
		Ok(())
	}
}

#[cfg(feature = "kafka")]
impl<B: BlockT + Unpin> MetadataActor<B> {
	pub async fn new(
		addr: Address<ActorPool<DatabaseActor<B>>>,
		meta: Meta<B>,
		kafka: Option<Address<KafkaActor<B>>>,
	) -> Result<Self> {
		let conn = addr.send(GetState::Conn.into()).await?.await?.conn();
		Ok(Self { conn, addr, meta, kafka })
	}

	// checks if the metadata exists in the database
	// if it doesn't exist yet, fetch metadata and insert it
	async fn meta_checker(
		&mut self,
		ver: u32,
		hash: B::Hash,
		block_num: <<B as BlockT>::Header as HeaderT>::Number,
	) -> Result<()> {
		if !queries::check_if_meta_exists(ver, &mut self.conn).await? {
			let meta = self.meta.clone();
			log::info!("Getting metadata for hash {}, version {}", hex::encode(hash.as_ref()), ver);
			let meta = smol::unblock(move || meta.metadata(&BlockId::hash(hash))).await?;
			let meta: sp_core::Bytes = meta.into();
			let meta = Metadata::new(ver, meta.0);
			self.addr.send(meta.clone().into()).await?.await;
			#[cfg(feature = "kafka")]
			if let Some(kafka) = &self.kafka {
				kafka.send(KafkaMetadataValue::from(meta, block_num, hash)).await?;
			}
		}
		Ok(())
	}

	async fn block_handler(&mut self, blk: Block<B>) -> Result<()>
	where
		NumberFor<B>: Into<u32>,
	{
		let hash = blk.inner.block.hash();
		let block_num = *blk.inner.block.header().number();
		self.meta_checker(blk.spec, hash, block_num).await?;
		self.addr.send(blk.clone().into()).await?.await;
		#[cfg(feature = "kafka")]
		if blk.inner.block.header().number() == Default::default() {
			if let Some(kafka) = &self.kafka {
				kafka.send(KafkaBlockValue::from(blk.inner.block, blk.spec, false, vec![])).await?;
			}
		}
		Ok(())
	}

	async fn batch_block_handler(&mut self, blks: BatchBlock<B>) -> Result<()>
	where
		NumberFor<B>: Into<u32>,
	{
		for blk in blks.inner().iter().unique_by(|&blk| blk.spec) {
			let block_num = *blk.inner.block.header().number();
			self.meta_checker(blk.spec, blk.inner.block.hash(), block_num).await?;
		}
		self.addr.send(blks.clone().into()).await?.await;
		Ok(())
	}
}

impl<B: BlockT> Actor for MetadataActor<B> {}

#[async_trait::async_trait]
impl<B> Handler<Block<B>> for MetadataActor<B>
where
	B: BlockT + Unpin,
	NumberFor<B>: Into<u32>,
{
	async fn handle(&mut self, blk: Block<B>, _: &mut Context<Self>) {
		if let Err(e) = self.block_handler(blk).await {
			log::error!("{}", e.to_string());
		}
	}
}

#[async_trait::async_trait]
impl<B> Handler<BatchBlock<B>> for MetadataActor<B>
where
	B: BlockT + Unpin,
	NumberFor<B>: Into<u32>,
{
	async fn handle(&mut self, blks: BatchBlock<B>, _: &mut Context<Self>) {
		if let Err(e) = self.batch_block_handler(blks).await {
			log::error!("{}", e.to_string());
		}
	}
}

#[async_trait::async_trait]
impl<B: BlockT + Unpin> Handler<Die> for MetadataActor<B>
where
	NumberFor<B>: Into<u32>,
	B::Hash: Unpin,
{
	async fn handle(&mut self, _: Die, ctx: &mut Context<Self>) {
		ctx.stop();
	}
}
