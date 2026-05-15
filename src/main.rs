fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dlna_rs=info".into()),
        )
        .init();
    let cfg = dlna_rs::config::Config::load_or_default(&dlna_rs::config::Config::default_path())?;
    tracing::info!(port = cfg.port, friendly_name = %cfg.friendly_name, folders = cfg.folders.len(), "loaded config");
    Ok(())
}
