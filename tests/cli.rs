#![cfg(feature = "gui")]

#[test]
fn help_and_version_work_without_starting_a_desktop() {
    for flag in ["--help", "-h", "--version", "-V"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_antodo"))
            .arg(flag)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        if flag.contains("help") || flag == "-h" {
            assert!(text.contains("FILE.antodo"));
        } else {
            assert!(text.contains(env!("CARGO_PKG_VERSION")));
        }
    }
}
