# Secrets and Encryption

A configuration repository is read by more people and machines than the server itself: the
Git remote, its backups, every clone. confect keeps key material out of it in two ways: files
matching `encrypt` patterns are stored encrypted with [age](https://age-encryption.org), and a
secret guard refuses to store recognizable secrets in plaintext.

## The secret guard

Whenever confect is about to store a new or changed file in plaintext (`sync`, `add`,
`category` changes), it checks the content for:

| Format | Example |
|--------|---------|
| PEM private keys | `-----BEGIN PRIVATE KEY-----`, `-----BEGIN RSA PRIVATE KEY-----`, `-----BEGIN EC PRIVATE KEY-----` |
| OpenSSH private keys | `-----BEGIN OPENSSH PRIVATE KEY-----` |
| PGP private keys | `-----BEGIN PGP PRIVATE KEY BLOCK-----` |
| age identities | `AGE-SECRET-KEY-1...` |
| PuTTY private keys | `PuTTY-User-Key-File-...` |
| JSON Web Keys with a private part | `{"kty": "RSA", ..., "d": "..."}` (certbot/ACME account keys) |
| SCRAM verifiers | `SCRAM-SHA-256$4096:...` (PostgreSQL, PgBouncer) |
| MD5 password hashes | `"user" "md5<32 hex digits>"` (PgBouncer `userlist.txt`) |
| crypt password hashes | `user:$6$...`, `$5$`, `$y$`, `$7$`, `$2a$/$2b$/$2y$`, `$apr1$`, `{SHA}` (`/etc/shadow`, htpasswd, squid) |
| GnuPG private keys | `(private-key ...`, `(protected-private-key ...`, anything under `private-keys-v1.d` |
| key stores, by file name | `*.p12`, `*.pfx`, `*.jks`, `*.keystore`, `*.kdbx`, `secring.gpg` |

If one is found, nothing is written and the command exits with `1`:

```
Plaintext secrets would be stored in the repository:
  ! /etc/nginx/ssl/site.key (PEM private key, category 'nginx')
  ! /etc/nginx/htpasswd (password hash (htpasswd/shadow format), category 'nginx')

Store them encrypted:   confect category encrypt add <category> <path>
or do not track them:   confect category exclude add <category> <path>
or accept the risk:     confect category allow-plaintext add <category> <path>
Error: 2 file(s) look like plaintext secrets; nothing was written (see above)
```

`status` shows the same report without failing. Resolve each file:

```bash
confect category encrypt add nginx /etc/nginx/ssl/site.key
confect category exclude add nginx /etc/nginx/htpasswd
confect sync
```

Limits of the guard:

- It recognizes formats that are secret by construction. Passwords and tokens inside
  ordinary configuration files (`password = ...`, WireGuard `PrivateKey = ...`, API tokens)
  are not detected. Put such files under `encrypt` yourself.
- Hash lines in comments (`#`, `;`) are ignored; key markers count anywhere. Whole files are
  read, binary ones included.
- It checks files whose content changes. A plaintext copy that is already in the repository
  (for example from confect 1.x) stays until you act; find those with
  [`confect audit`](/commands/audit).

## Encrypting files

Create a key once per machine:

```bash
confect key generate
```

This writes the identity (private key) to `~/.config/confect/age-identity.txt` with mode
`0600` and its public key to `~/.config/confect/age-recipients.txt`. Both locations can be
changed in `[encryption]` in the [global configuration](/guide/configuration).

::: danger Back up the identity
Encrypted files can only be read with the identity. Copy `age-identity.txt` somewhere safe
outside the machine, such as a password manager or an offline medium. Without it, a rebuilt
host cannot restore its encrypted files.
:::

Then mark files for encryption with `encrypt` patterns:

```bash
confect category encrypt add nginx '*.key'
confect category encrypt add base /etc/ssh/ssh_host_ed25519_key
confect add /etc/wireguard -c vpn --create-category --encrypt
```

Matching files are re-stored encrypted right away and committed by the next `sync`. In the
repository they get an `.age` suffix, for example
`nginx/etc/nginx/ssl/site.key.age`. Mode, owner and group are kept in
`.confect/metadata.toml` as for any other file.

- `status` and `diff` decrypt the stored copy to compare it with the system file.
- `restore` decrypts and writes the plaintext back with the recorded mode and owner.
- Directories and symlinks are never encrypted; only regular files are.
- A stored copy that cannot be decrypted (damaged, or written for a key this host does not
  have) is never replaced silently: `sync` keeps it and warns. After replacing the age key on
  purpose, `confect sync --reencrypt` encrypts every file again from the system.
- An encrypted `x` is stored as `x.age`. If the same category also tracks a plain file named
  `x.age`, both would land on one stored path; confect refuses and asks you to exclude one.

::: warning
A file that was committed in plaintext before you added the `encrypt` pattern is still in the
Git history. Rotate that secret; see [audit](/commands/audit).
:::

## Hosts without the identity

Encrypting only needs the recipients file. A host that has `age-recipients.txt` but no
identity can still `sync`: it notices changes to encrypted files by their size and
modification time, and re-encrypts them. It cannot `diff` or `restore` them;
`confect key show` reports the identity as missing.

## More than one recipient

The recipients file may list several `age1...` public keys, one per line (`#` comments are
allowed). Every file is then encrypted for all of them. A common setup is the host's own key
plus an offline recovery key:

```
age1qdfxd9lp9cwth2u0rp69kpr0jhjph80qwsjlr00fzdnzrszd4yfqlq6ckd
# offline recovery key
age1ermz6qfwfcq02jy7dzsuw5gfcng9s2qlg699h94y3zjllcltzg2sgyp2ae
```

Adding a recipient does not re-encrypt existing copies; each file picks up the new
recipient the next time it changes.

## Rebuilding a host with encrypted files

Put the identity and the recipients file back before restoring:

```bash
mkdir -p ~/.config/confect
cp /media/backup/age-identity.txt /media/backup/age-recipients.txt ~/.config/confect/
chmod 600 ~/.config/confect/age-identity.txt
confect init --system --from git@git.example.com:ops/configs.git --host web1
confect restore --yes
```

Do not run `confect key generate` on the rebuilt host: a new key cannot read the old files.
