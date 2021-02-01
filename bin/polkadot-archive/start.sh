ulimit -n 100000
export RUST_BACKTRACE=full
nohup ./target/release/polkadot-archive -c polkadot.toml --spec=polkadot > output.log 2>&1 &
