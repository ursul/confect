# Encryption

confect uses [age](https://github.com/FiloSottile/age) for encrypting sensitive configuration files.

## How it works

1. When you add a file with `--encrypt`, confect generates an age key pair
2. The file is encrypted before being stored in the repository
3. On restore, the file is decrypted using your private key

## Key storage

Keys are stored in `~/.config/confect/`:

```
~/.config/confect/
├── age-key.txt      # Private key (never commit!)
└── age-pubkey.txt   # Public key
```

::: warning
Never commit your private key to the repository. The `.gitignore` should exclude `~/.config/confect/`.
:::

## Adding encrypted files

```bash
confect add --encrypt ~/.ssh/config
confect add --encrypt ~/.aws/credentials
```

## Viewing encrypted status

```bash
confect status
```

Encrypted files are marked with `E`:

```
  E ~/.ssh/config     (encrypted, synced)
```

## Sharing encrypted configs

To use encrypted configs on another machine:

1. Copy your `age-key.txt` to the new machine
2. Run `confect restore`

Or use age's native key sharing with recipients.
