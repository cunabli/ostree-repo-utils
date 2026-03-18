## ADDED Requirements

### Requirement: Export-key subcommand accepts provider flag
The `ostree-repo-server export-key` subcommand SHALL accept `--provider=<name>` as a required flag. For this change, only `kms` is a valid provider value.

#### Scenario: Valid KMS provider accepted
- **WHEN** `ostree-repo-server export-key --provider=kms --key-id <ARN> --key-version <VERSION> --keys-dir <PATH>` is invoked
- **THEN** the command SHALL proceed to export the public key

### Requirement: Public key extracted from KMS DER SPKI response
The export-key command SHALL call AWS KMS `GetPublicKey` for the given `--key-id`. The response is a DER-encoded SubjectPublicKeyInfo (SPKI) structure. The command SHALL extract the raw 32-byte ED25519 public key by validating and stripping the fixed 12-byte ASN.1 header (`30 2a 30 05 06 03 2b 65 70 03 21 00`).

#### Scenario: Valid ED25519 SPKI parsed successfully
- **WHEN** KMS returns a well-formed ED25519 SPKI blob
- **THEN** the command SHALL extract the trailing 32 bytes as the raw public key

#### Scenario: Unexpected SPKI structure rejected
- **WHEN** the returned DER blob does not begin with the expected ED25519 ASN.1 header
- **THEN** the command SHALL exit non-zero with an error describing the unexpected format

### Requirement: Public key written in ostree keys-dir format
The raw 32-byte public key SHALL be base64-encoded and written as a single line to `<keys-dir>/<key-version>.pub`. This file format SHALL be compatible with ostree's `--keys-dir` option for ED25519 verification.

#### Scenario: Key file written to correct path
- **WHEN** `--keys-dir /etc/ostree/trusted.ed25519.d/` and `--key-version prod-v1` are passed
- **THEN** the file `/etc/ostree/trusted.ed25519.d/prod-v1.pub` SHALL be created containing a single base64-encoded line

#### Scenario: Exported key enables offline verification
- **WHEN** the exported `.pub` file is present in a `--keys-dir` directory
- **THEN** `ostree sign --verify --sign-type=ed25519 --keys-dir=<dir>` SHALL accept signatures produced by the corresponding KMS key

#### Scenario: Existing file overwritten
- **WHEN** a file at the target path already exists
- **THEN** the command SHALL overwrite it without error (key rotation use case)

### Requirement: Keys-dir path is operator-supplied
The `--keys-dir` argument SHALL be required with no default. The command SHALL not hardcode any system path.

#### Scenario: Missing keys-dir argument rejected
- **WHEN** `--keys-dir` is omitted
- **THEN** the command SHALL exit non-zero with a usage error
