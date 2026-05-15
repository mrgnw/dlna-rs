use dlna_rs::config::Config;

#[test]
fn parse_roundtrip() {
    let input = r#"
port 9119
friendly_name test
folders ({path "/tmp/a", recursive true} {path "/tmp/b", recursive false})
"#;
    let cfg: Config = facet_styx::from_str(input).unwrap();
    assert_eq!(cfg.port, 9119);
    assert_eq!(cfg.folders.len(), 2);
    assert_eq!(cfg.folders[0].path, "/tmp/a");
    assert_eq!(cfg.folders[0].recursive, true);
    assert_eq!(cfg.folders[1].recursive, false);

    let out = facet_styx::to_string(&cfg).unwrap();
    let cfg2: Config = facet_styx::from_str(&out).unwrap();
    assert_eq!(cfg, cfg2);
}

#[test]
fn default_friendly_name_templates_hostname() {
    let cfg = Config::with_defaults();
    assert!(cfg
        .friendly_name
        .contains(&gethostname::gethostname().to_string_lossy().to_string()));
}
