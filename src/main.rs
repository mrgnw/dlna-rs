use clap::Parser;
use dlna_rs::{
	api::{self, AppState},
	browse::LibraryProvider,
	cli::Cli,
	config::Config,
	library::Library,
	stream, web,
};
use librqbit_upnp_serve::{UpnpServer, UpnpServerOptions};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| "dlna_rs=info,librqbit_upnp_serve=info".into()),
		)
		.init();

	let cli = Cli::parse();
	let cfg_path = cli.config.clone().unwrap_or_else(Config::default_path);
	let mut cfg = Config::load_or_default(&cfg_path)?;
	cli.apply(&mut cfg);

	let lib = Arc::new(RwLock::new(Library::from_config(&cfg.folders)));
	let state = AppState { lib: lib.clone(), cfg_path: Arc::new(cfg_path) };

	let cancel = CancellationToken::new();
	let mut upnp = UpnpServer::new(UpnpServerOptions {
		friendly_name: cfg.friendly_name.clone(),
		http_listen_port: cfg.port,
		http_prefix: String::new(),
		browse_provider: Box::new(LibraryProvider(lib.clone())),
		cancellation_token: cancel.clone(),
	})
	.await?;
	let upnp_router = upnp.take_router()?;

	let app = axum::Router::new()
		.merge(upnp_router)
		.merge(api::router(state.clone()))
		.merge(stream::router(state.clone()))
		.merge(web::router());

	let addr = format!("0.0.0.0:{}", cfg.port);
	tracing::info!(%addr, friendly = %cfg.friendly_name, "http listening");
	let listener = tokio::net::TcpListener::bind(&addr).await?;

	let ssdp_handle = tokio::spawn(async move {
		if let Err(e) = upnp.run_ssdp_forever().await {
			tracing::warn!("ssdp loop ended: {e}");
		}
	});

	let server = tokio::spawn(async move { axum::serve(listener, app).await });

	tokio::select! {
		_ = tokio::signal::ctrl_c() => {
			tracing::info!("ctrl-c, shutting down");
			cancel.cancel();
		}
		r = server => { r??; }
	}
	let _ = ssdp_handle.await;
	Ok(())
}
