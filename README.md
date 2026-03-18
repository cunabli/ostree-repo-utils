# ostree-repo-utils

[![CI](https://github.com/cunabli/ostree-repo-utils/workflows/CI/badge.svg)](https://github.com/cunabli/ostree-repo-utils/actions)

Utilities for managing [OSTree](https://ostreeproject.org/) repositories in production release pipelines.

The core capability today is **signing OSTree commits with ED25519 keys stored in AWS KMS** and exporting the corresponding public key for fully offline verification on client systems — no KMS access required at verification time.

## Binaries

| Binary | Crate | Audience |
|--------|-------|----------|
| `ostree-repo-server` | `ostree-repo-server-utils` | Build infrastructure, CI/CD pipelines |
| `ostree-repo-client` | `ostree-repo-client-utils` | Client / target systems |

The client binary carries **zero AWS SDK dependencies**. The server binary is the only component that requires KMS access.

## Prerequisites

- **Rust 1.92+** — the minimum supported Rust version (MSRV).
- **libostree** ≥ 2024.05 and its development headers (`ostree-1.pc` must be discoverable by `pkg-config`).
  - Fedora/RHEL: `dnf install ostree-devel`
  - Debian/Ubuntu: `apt install libostree-dev`
  - Yocto: included when building with the `ostree` recipe.
- **AWS credentials** (for the server binary only) — any credential source supported by the AWS SDK works: environment variables, instance role, `~/.aws/credentials`, etc.

## Installation

```sh
cargo install --git https://github.com/cunabli/ostree-repo-utils ostree-repo-server-utils
cargo install --git https://github.com/cunabli/ostree-repo-utils ostree-repo-client-utils
```

Or build from source:

```sh
git clone https://github.com/cunabli/ostree-repo-utils
cd ostree-repo-utils
cargo build --release
# Binaries land at:
#   target/release/ostree-repo-server
#   target/release/ostree-repo-client
```

## Usage

### Sign a commit

Sign the commit at the tip of a branch with a KMS-backed ED25519 key:

```sh
ostree-repo-server sign \
  --provider kms \
  --key-id alias/my-ed25519-key \
  --repo /var/lib/ostree \
  --ref main
```

By default, signatures are **appended** — existing signatures on the commit are preserved. To discard all prior signatures and store only the new one (e.g. after revoking a key):

```sh
ostree-repo-server sign \
  --provider kms \
  --key-id alias/my-ed25519-key \
  --repo /var/lib/ostree \
  --ref main \
  --replace
```

### Export the public key

Write the KMS public key to a directory so that client systems can verify signatures offline:

```sh
ostree-repo-server export-key \
  --provider kms \
  --key-id alias/my-ed25519-key \
  --key-version prod-v1 \
  --keys-dir /etc/ostree/trusted.ed25519.d
# Writes: /etc/ostree/trusted.ed25519.d/prod-v1.pub
```

### Verify a signed commit (client side)

Verification uses the native `ostree` CLI — no custom binary required on client systems:

```sh
ostree sign --verify \
  --sign-type ed25519 \
  --keys-dir /etc/ostree/trusted.ed25519.d \
  --repo /sysroot/ostree/repo \
  <commit-checksum>
```

### Required IAM permissions

The AWS IAM principal running `ostree-repo-server` needs:

```json
{
  "Effect": "Allow",
  "Action": ["kms:Sign", "kms:GetPublicKey"],
  "Resource": "arn:aws:kms:<region>:<account>:key/<key-id>"
}
```

## How it works

1. `sign` loads the serialized GVariant bytes of the OSTree commit object and sends them to KMS using the `ED25519_SHA_512` algorithm with `MessageType::Raw`. KMS returns a 64-byte signature.
2. The signature is stored in the commit's detached metadata under the key `ostree.sign.ed25519` as a GVariant of type `(aay)` — the exact format expected by `ostree sign --verify`.
3. `export-key` calls KMS `GetPublicKey`, strips the 12-byte DER SPKI header from the response, and writes the raw 32-byte public key base64-encoded to a `.pub` file.

## License

Licensed under MIT license ([LICENSE](LICENSE) or <http://opensource.org/licenses/MIT>)

at your option.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).
