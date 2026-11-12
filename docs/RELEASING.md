# Packaging and releasing antodo

The project is configured as a Cargo binary crate, version `1.0.0`, with GPL-2.0-only licensing and repository metadata for the intended `tothaa/antodo` repository. Package preparation does not require a crates.io account. Publishing does.

## Check a local release

Use a current stable Rust toolchain. Run the automated checks and native desktop checklist in [REQUIREMENTS.md](REQUIREMENTS.md). Review the actual output; do not substitute compilation for visual or file-dialog verification.

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --no-default-features
cargo llvm-cov --no-default-features --summary-only
cargo llvm-cov --all-features --summary-only
cargo build --release --locked
cargo package --list
cargo package --locked
cargo install --path . --locked
```

`cargo package` creates `target/package/antodo-1.0.0.crate` and builds its extracted contents to verify that the published source is complete. Keep the working tree clean first. `--allow-dirty` is useful for local preparation, but a release should correspond to a reviewed commit. Do not use `--no-verify` as a substitute for successful package verification.

The manifest includes source, tests, documentation, assets, Cargo metadata, README, AGENTS, changelog and license. It excludes private `.antodo` databases, build outputs and generated coverage files. Inspect `cargo package --list` before publishing. Commit `Cargo.lock` for this application so local and CI builds can use `--locked`.

The application bundles SQLite. Linux builds use Iced's software renderer with X11/Wayland support and portal-backed file dialogs. Check the target desktop's runtime libraries and portal backend. The supplied `assets/antodo.desktop` and `assets/antodo.svg` provide the matching Wayland taskbar identity; `sh assets/install-desktop.sh` installs both for the current user. Cargo installs the binary, not a system desktop package. A distributor can install the desktop file and icon through the normal package-manager paths.

## Share on GitHub later

The Git author name for this project is `tothaa`. Before committing, configure the email address or GitHub-provided private commit email you want associated with your commits; do not assume a private address from the username.

Create an empty repository under your GitHub account when ready. Then, if no origin remote exists:

```sh
git remote add origin https://github.com/tothaa/antodo.git
git push -u origin main
```

If origin already exists, inspect it with `git remote -v` first. The repository URL in Cargo metadata is an intended destination and does not by itself create a GitHub repository. CI is a checked-in workflow and only runs on GitHub after the repository is pushed and Actions is enabled.

## Publish to crates.io later

1. Resolve the dependency licensing compatibility issue before distributing a combined GPL-2.0-only release, then create/sign in to a crates.io account and verify its requirements.
2. Check availability of the exact crate name `antodo`. Rename the crate and update documentation if another owner has already registered it.
3. Ensure the public source URL, license, version, README and changelog match the release. Audit the resolved dependency licenses for the distribution you intend to make.
4. Authenticate with `cargo login` using your own token; keep it out of Git and screenshots.
5. Run `cargo publish --dry-run --locked` and inspect its verification output.
6. When you explicitly decide to release, run `cargo publish --locked`.

After a successful public release, users can run:

```sh
cargo install antodo --locked
```

Crates.io publication is an external release action. No publication, remote repository creation, push, or account configuration is performed by these files. See the official [Cargo publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html) for the current registry process.

## Versioning and database changes

Update `Cargo.toml` and `CHANGELOG.md` for each release, regenerate `Cargo.lock` as needed, and rerun package verification. Future schema changes must have explicit, transactional migrations and regression fixtures. Never silently accept an unknown `PRAGMA user_version`; preserve the original file if a migration fails. Schema version 4 is current; opening version 1, 2 or 3 migrates its in-memory copy, and the original file changes only on Save.
