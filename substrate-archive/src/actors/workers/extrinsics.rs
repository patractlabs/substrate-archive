use super::ActorPool;
use codec::{Decode, Encode};
use polkadot_runtime::UncheckedExtrinsic;
///  注意，这里
/// 1. 只支持Ｐｏｌｋａｄｏｔ，不是通用的
/// 2. 只支持最新的Ｐｏｌｋａｄｏｔ　runtime 版本解析，抛弃了历史区块中的被删除的ｍｏｄｕｌｅ的交易
///
use sp_runtime::traits::{Block as BlockT, Header as _, NumberFor};
use substrate_archive_common::{
	types::{BatchBlock, BatchExtrinsic, Block, Extrinsic},
	Result,
};
use xtra::prelude::*;

#[derive(Clone)]
pub struct ExtrinsicsActor<B: BlockT> {
	addr: Address<ActorPool<super::DatabaseActor<B>>>,
}

impl<B: BlockT> ExtrinsicsActor<B>
where
	B: BlockT,
	NumberFor<B>: Into<u32>,
	B::Hash: Unpin,
{
	pub fn new(addr: Address<ActorPool<super::DatabaseActor<B>>>) -> Self {
		Self { addr }
	}

	async fn extrinsics_handler(&self, blk: Block<B>) -> Result<()> {
		let hash = blk.inner.block.header().hash();
		let block_num: u32 = (*blk.inner.block.header().number()).into();
		let mut extrinsics_list: Vec<Extrinsic<B>> = Vec::new();
		let extrinsics: &[B::Extrinsic] = blk.inner.block.extrinsics();

		for index in 0..extrinsics.len() {
			let extrinsic = &extrinsics[index];
			let _ = UncheckedExtrinsic::decode(&mut extrinsic.encode().as_ref()).map(|e| {
				let function = e.function;
				match e.signature {
					Some((address, signature, extra)) => {
						extrinsics_list.push(Extrinsic::new_signed(
							hash,
							block_num,
							index as u32,
							address.encode(),
							signature.encode(),
							extra.encode(),
							function.encode(),
						));
					}
					None => {
						extrinsics_list.push(Extrinsic::new_unsigned(hash, block_num, index as u32, function.encode()));
					}
				}
			});
		}

		self.addr.send(BatchExtrinsic::new(extrinsics_list).into());
		Ok(())
	}
}

impl<B: BlockT> Actor for ExtrinsicsActor<B> {}

#[async_trait::async_trait]
impl<B: BlockT> Handler<Block<B>> for ExtrinsicsActor<B>
where
	B: BlockT,
	NumberFor<B>: Into<u32>,
	B::Hash: Unpin,
{
	async fn handle(&mut self, blk: Block<B>, _: &mut Context<Self>) {
		if let Err(e) = self.extrinsics_handler(blk).await {
			log::error!("{}", e.to_string())
		}
	}
}

#[async_trait::async_trait]
impl<B> Handler<BatchBlock<B>> for ExtrinsicsActor<B>
where
	B: BlockT,
	NumberFor<B>: Into<u32>,
	B::Hash: Unpin,
{
	async fn handle(&mut self, blks: BatchBlock<B>, _: &mut Context<Self>) {
		let len = blks.inner.len();
		let now = std::time::Instant::now();
		for blk in blks.inner {
			if let Err(e) = self.extrinsics_handler(blk).await {
				log::error!("{}", e.to_string())
			}
		}
		if len > 1000 {
			log::info!("took {:?} to insert {} blocks extrinsics", now.elapsed(), len);
		} else {
			log::debug!("took {:?} to insert {} blocks extrinsics", now.elapsed(), len);
		}
	}
}

#[async_trait::async_trait]
impl<B: BlockT + Unpin> Handler<super::Die> for ExtrinsicsActor<B>
where
	NumberFor<B>: Into<u32>,
	B::Hash: Unpin,
{
	async fn handle(&mut self, _: super::Die, ctx: &mut Context<Self>) -> Result<()> {
		ctx.stop();
		Ok(())
	}
}
