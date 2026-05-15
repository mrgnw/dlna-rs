use std::path::PathBuf;
use std::process::Command;

fn main() {
	let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let web = manifest.join("web");
	if !web.exists() {
		println!("cargo:warning=web/ missing; skipping bundle build");
		return;
	}
	println!("cargo:rerun-if-changed=web/package.json");
	println!("cargo:rerun-if-changed=web/vite.config.js");
	println!("cargo:rerun-if-changed=web/index.html");
	println!("cargo:rerun-if-changed=web/src");

	if !web.join("node_modules").exists() {
		let status = Command::new("bun")
			.arg("install")
			.current_dir(&web)
			.status();
		match status {
			Ok(s) if s.success() => {}
			_ => {
				println!("cargo:warning=bun install failed; web UI may be missing");
				return;
			}
		}
	}
	let status = Command::new("bun")
		.args(["run", "build"])
		.current_dir(&web)
		.status();
	match status {
		Ok(s) if s.success() => {}
		_ => println!("cargo:warning=bun run build failed; web UI may be missing"),
	}
}
