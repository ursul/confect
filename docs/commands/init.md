# init

Create a repository, or clone an existing one and continue this host's branch in it.

## Usage

```bash
confect init [OPTIONS]
```

## Options

| Option | Description |
|--------|-------------|
| `-p`, `--path <PATH>` | Repository directory (default: `~/.local/share/confect`) |
| `--system` | Use the system-wide repository `/var/lib/confect` |
| `--remote <URL>` | Remote to push to |
| `--from <URL>` | Clone this repository and continue this host's branch in it |
| `--host <NAME>` | Host name; the branch is `host/<NAME>` (default: the hostname) |

`--path` and `--system` exclude each other, as do `--remote` and `--from`. The global
`--repo` option also works as the path.

## New repository

```bash
sudo confect init --system --remote git@git.example.com:ops/configs.git
```

- creates the directory with mode `0700` and a Git repository on the branch `host/<name>`;
- writes `.confect/config.toml` (format version, host name), an empty
  `.confect/categories.toml` and `.confect/metadata.toml`;
- commits `Initialize confect repository for <name>`;
- adds the remote under the name `default_remote` (`origin`).

Nothing is pushed yet; the first `sync` does that. When the path is not the default one
(`--path`, `--system`), it is saved as `[global] repo_path`, so later commands find the
repository without options.

## Clone for this host

```bash
sudo confect init --system --from git@git.example.com:ops/configs.git --host web1
```

The target directory must be empty or not exist. confect clones the repository, then:

- if the remote has `host/web1`, checks it out and tracks it; `confect restore` writes those
  files to this machine;
- otherwise starts `host/web1` as a new empty branch (commit
  `Start configuration of web1`), without the files of other hosts.

This is the way to rebuild a machine and to add a new host to a shared remote. See
[Multiple Hosts](/advanced/multi-host).

## Add a remote later

On an existing repository, `init --remote` only adds the remote:

```bash
confect init --remote git@git.example.com:ops/configs.git
```

```
✓ Added remote origin: git@git.example.com:ops/configs.git
```

If the remote is already set, confect refuses and shows the `git remote set-url` command to
change it. Without `--remote`, `init` on an existing repository fails with
`Repository already initialized`.

## Host names

Letters, digits, `.`, `_` and `-`; not starting with `.` or `-`, not ending with `.`, no `..`.
The name is stored in the repository, so changing the machine's hostname later does not
change the branch.
