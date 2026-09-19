# audit

Look for plaintext secrets in the repository.

## Usage

```bash
confect audit [--history]
```

## Options

| Option | Description |
|--------|-------------|
| `--history` | Search every commit, not only the current files |

## What it does

Without options, `audit` checks every file in the repository's working tree. With
`--history`, it checks every commit reachable from any branch in the local repository,
including the other hosts' branches that a clone brought along. Encrypted (`.age`) files are
skipped. It recognizes the same formats as the [secret guard](/advanced/encryption#the-secret-guard).

```
1 file(s) with plaintext secrets in history:
  ! nginx/etc/nginx/ssl/site.key (PEM private key) in 2 commit(s): 982d3d4e424a, 7aa50ddad7c0

Treat these secrets as disclosed to everyone who can read the repository:
rotate them, then encrypt or exclude the files. Removing them from history
needs a history rewrite and a force-push, which confect does not do for you.
```

Paths are relative to the repository: category directory, then the system path.

| Exit code | Meaning |
|-----------|---------|
| `0` | Nothing found |
| `3` | Plaintext secrets found (files allowed with `allow_plaintext` do not count) |
| `1` | Error |

`audit` also works on 1.x repositories, before [`migrate`](/commands/migrate).

## When something is found

1. Rotate the secret: a new key, password or certificate. Everyone who could read the
   repository or its remote may have a copy of the old one.
2. Stop storing it in plaintext, then sync:

   ```bash
   confect category encrypt add nginx '*.key'
   confect sync
   ```

   or leave it out with `confect category exclude add ...`.
3. `confect audit` should now be clean. `confect audit --history` keeps reporting the old
   commits until you rewrite the history yourself and force-push every affected branch.

Files allowed with `allow_plaintext` are reported too: the allowance only affects the
secret guard of `sync`.
