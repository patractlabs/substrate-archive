// Copyright 2018-2019 Parity Technologies (UK) Ltd.
// This file is part of substrate-archive.

// substrate-archive is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// substrate-archive is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of // MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with substrate-archive.  If not, see <http://www.gnu.org/licenses/>.

mod cli_opts;

use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc,
};

use anyhow::{anyhow, Result};
use sp_runtime::{generic, traits::BlakeTwo256, OpaqueExtrinsic};
use substrate_archive::{
	native_executor_instance, Archive, ArchiveBuilder, ArchiveConfig, ReadOnlyDB, SecondaryRocksDB,
};

use jupiter_runtime as jupiter_rt;

native_executor_instance!(
	pub JupiterExecutor,
	jupiter_rt::api::dispatch,
	jupiter_rt::native_version,
	(sp_io::SubstrateHostFunctions, patract_io::pairing::HostFunctions),
);

type BlockNumber = u32;
type Header = generic::Header<BlockNumber, BlakeTwo256>;
type Block = generic::Block<Header, OpaqueExtrinsic>;

type ChainSpec = sc_service::GenericChainSpec<jupiter_rt::GenesisConfig>;

pub fn main() -> Result<()> {
	let cli = cli_opts::CliOpts::init();
	let config = cli.parse()?;

	let mut archive = run_archive::<SecondaryRocksDB>(&cli.chain_spec, config)?;
	archive.drive()?;
	let running = Arc::new(AtomicBool::new(true));
	let r = running.clone();

	ctrlc::set_handler(move || {
		r.store(false, Ordering::SeqCst);
	})
	.expect("Error setting Ctrl-C handler");
	while running.load(Ordering::SeqCst) {}
	archive.boxed_shutdown()?;

	Ok(())
}

fn run_archive<D: ReadOnlyDB + 'static>(
	chain_spec: &str,
	config: Option<ArchiveConfig>,
) -> Result<Box<dyn Archive<Block, D>>> {
	match chain_spec.to_ascii_lowercase().as_str() {
		"jupiter" => {
			let spec = jupiter_poa()?;
			let archive = ArchiveBuilder::<Block, jupiter_rt::RuntimeApi, JupiterExecutor, D>::with_config(config)
				.chain_spec(Box::new(spec))
				.build()?;
			Ok(Box::new(archive))
		}
		c => Err(anyhow!("unknown chain {}", c)),
	}
}

fn jupiter_poa() -> Result<ChainSpec> {
	let spec =
		ChainSpec::from_json_bytes(&include_bytes!("../jupiter_poa.json")[..]).map_err(|err| anyhow!("{}", err))?;
	Ok(spec)
}
