# key

Manage the age key used for encrypted files.

## Usage

```bash
confect key generate
confect key show
```

## generate

Creates an X25519 age identity and adds its public key to the recipients file:

```
✓ Created /root/.config/confect/age-identity.txt (keep a copy outside this machine: without it encrypted files are lost)
✓ Added age1qdfxd9lp9cwth2u0rp69kpr0jhjph80qwsjlr00fzdnzrszd4yfqlq6ckd to /root/.config/confect/age-recipients.txt
```

- The identity file is created with mode `0600` and never overwritten: if it exists,
  `generate` fails. Move the old file away first if you really want a new key; files
  encrypted for the old key then need the old identity to be read.
- An existing recipients file is kept; the new public key is appended.
- The locations come from `[encryption] identity_file` and `recipients_file` in the
  [global configuration](/guide/configuration). The repository is not involved.

## show

Prints the recipients that encrypted files are written for, and whether the identity is
present:

```
age1qdfxd9lp9cwth2u0rp69kpr0jhjph80qwsjlr00fzdnzrszd4yfqlq6ckd
i identity /root/.config/confect/age-identity.txt: present
```

`missing (this host can encrypt but not decrypt)` means the host can still `sync` encrypted
files but cannot `diff` or `restore` them.

## Recipients file

One age public key (`age1...`) per line; empty lines and `#` comments are ignored. Every
encrypted file is written for all listed recipients, so you can add an offline recovery key
next to the host's own key. Only native age X25519 keys are supported.

See [Secrets and Encryption](/advanced/encryption).
