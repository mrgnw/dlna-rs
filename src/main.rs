use clap::Parser;
use dlna_rs::{cli::Cli, config::Config, library::Library};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| "dlna_rs=info".into()),
		)
		.init();
	let cli = Cli::parse();
	let cfg_path = cli.config.clone().unwrap_or_else(Config::default_path);
	let mut cfg = Config::load_or_default(&cfg_path)?;
	cli.apply(&mut cfg);
	let lib = Arc::new(RwLock::new(Library::from_config(&cfg.folders)));
	tracing::info!(?cfg_path, port = cfg.port, folders = lib.read().await.folders().len(), "library ready");
	Ok(())
}
