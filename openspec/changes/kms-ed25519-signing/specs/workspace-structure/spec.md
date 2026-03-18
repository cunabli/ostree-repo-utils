## ADDED Requirements

### Requirement: Cargo workspace with three crates
The repository SHALL be structured as a Cargo workspace with three members: `ostree-repo-utils-common` (lib), `ostree-repo-server-utils` (binary crate producing `ostree-repo-server`), and `ostree-repo-client-utils` (binary crate producing `ostree-repo-client`). The root `Cargo.toml` SHALL be a workspace manifest only, with no `[package]` section.

#### Scenario: Client binary has no AWS dependencies
- **WHEN** `ostree-repo-client-utils` is compiled with default features
- **THEN** the resulting binary SHALL NOT transitively depend on `aws-sdk-kms`, `aws-config`, or `tokio`

#### Scenario: Server binary enables KMS feature
- **WHEN** `ostree-repo-server-utils` is compiled
- **THEN** the `kms` feature of `ostree-repo-utils-common` SHALL be enabled and `ostree-repo-server` binary SHALL be the installed artifact

### Requirement: Feature-gated common library
`ostree-repo-utils-common` SHALL expose a `kms` feature flag. All AWS SDK dependencies and KMS-specific code SHALL be compiled only when this feature is active. The crate SHALL compile successfully without any features enabled.

#### Scenario: Common lib compiles without features
- **WHEN** `cargo build -p ostree-repo-utils-common` is run with no features
- **THEN** compilation SHALL succeed and produce no binary

#### Scenario: Common lib compiles with kms feature
- **WHEN** `cargo build -p ostree-repo-utils-common --features kms` is run
- **THEN** compilation SHALL succeed with `aws-sdk-kms` included

### Requirement: Rust MSRV compliance
All crates in the workspace SHALL compile with Rust 1.92 without requiring a newer toolchain.

#### Scenario: Workspace compiles on MSRV
- **WHEN** `cargo +1.92 build --workspace` is run
- **THEN** all crates SHALL compile without error
