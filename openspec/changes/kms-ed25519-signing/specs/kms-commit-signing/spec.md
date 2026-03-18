## ADDED Requirements

### Requirement: Sign subcommand accepts provider flag
The `ostree-repo-server sign` subcommand SHALL accept `--provider=<name>` as a required flag. For this change, only `kms` is a valid provider value. Unrecognised provider values SHALL produce a descriptive error and exit non-zero.

#### Scenario: Valid KMS provider accepted
- **WHEN** `ostree-repo-server sign --provider=kms --key-id <ARN> --repo <PATH> --ref <REF>` is invoked
- **THEN** the command SHALL proceed to sign the commit

#### Scenario: Unknown provider rejected
- **WHEN** `ostree-repo-server sign --provider=gpg ...` is invoked
- **THEN** the command SHALL exit non-zero with a message indicating the provider is not supported

### Requirement: Commit bytes signed via KMS ED25519_SHA_512
The sign command SHALL load the commit GVariant bytes for the resolved ref via `load_variant(OstreeObjectType::Commit, rev)`, then call the AWS KMS `Sign` API with algorithm `ED25519_SHA_512` and `MessageType::Raw`, passing the raw serialized bytes.

#### Scenario: Successful KMS signing
- **WHEN** the ref resolves to a valid commit and KMS credentials are available
- **THEN** KMS SHALL return a 64-byte raw ED25519 signature

#### Scenario: Commit message size guard
- **WHEN** the serialized commit GVariant exceeds 4096 bytes
- **THEN** the command SHALL exit non-zero with an error referencing the KMS 4096-byte limit before calling KMS

### Requirement: Signature stored in ostree detached metadata
The 64-byte KMS signature SHALL be written to the commit's detached metadata under key `"ostree.sign.ed25519"` with GVariant type `(aay)` (a tuple containing an array of byte arrays).

#### Scenario: Signature appended by default
- **WHEN** the commit already has one or more signatures under `ostree.sign.ed25519` and `--replace` is not set
- **THEN** the new signature SHALL be appended to the existing array and all prior signatures SHALL be preserved

#### Scenario: Replace flag clears existing signatures
- **WHEN** `--replace` is passed
- **THEN** the detached metadata for `ostree.sign.ed25519` SHALL contain only the new signature after the command completes

#### Scenario: Signature verifiable by native ostree CLI
- **WHEN** a commit has been signed and the corresponding public key is present in a `--keys-dir` directory
- **THEN** `ostree sign --verify --sign-type=ed25519 --keys-dir=<dir> --repo=<repo> <commit>` SHALL exit zero

### Requirement: Ref resolution
The `--ref` argument SHALL accept both symbolic refs (branch names) and full commit checksums. The command SHALL resolve symbolic refs to their commit hash before signing.

#### Scenario: Symbolic ref resolved
- **WHEN** `--ref main` is passed and `main` points to commit `abc123...`
- **THEN** the commit at `abc123...` SHALL be signed

#### Scenario: Invalid ref rejected
- **WHEN** `--ref` does not resolve to a commit in the given repo
- **THEN** the command SHALL exit non-zero with a descriptive error
