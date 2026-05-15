use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "dlna-rs", version, about = "Single-binary DLNA media server")]
pub struct Cli {
	#[arg(long)]
	pub config: Option<PathBuf>,
	#[arg(long)]
	pub port: Option<u16>,
	#[arg(long)]
	pub friendly_name: Option<String>,
	#[arg(long = "folder", value_name = "PATH")]
	pub folder: Vec<PathBuf>,
	#[arg(long = "folder-flat", value_name = "PATH")]
	pub folder_flat: Vec<PathBuf>,
}

impl Cli {
	pub fn apply(self, cfg: &mut crate::config::Config) {
		if let Some(p) = self.port {
			cfg.port = p;
		}
		if let Some(n) = self.friendly_name {
			cfg.friendly_name = n;
		}
		for p in self.folder {
			cfg.folders.push(crate::config::Folder {
				path: p.to_string_lossy().into_owned(),
				recursive: true,
			});
		}
		for p in self.folder_flat {
			cfg.folders.push(crate::config::Folder {
				path: p.to_string_lossy().into_owned(),
				recursive: false,
			});
		}
	}
}
