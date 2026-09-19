# Multiple Hosts

One remote repository can hold the configuration of many machines. Each host writes only its
own branch:

```
git@git.example.com:ops/configs.git
├── host/web1
├── host/web2
└── host/db1
```

A branch is the complete history of one machine. confect never merges branches and has no
shared base branch: `sync` and `push` update `host/<name>`, and `pull` fetches nothing else.

## Add a host

On the new machine, clone the shared remote under the new host's name:

```bash
sudo -i
confect init --system --from git@git.example.com:ops/configs.git --host web2
```

```
✓ The remote has no branch host/web2 yet; started it empty
```

Then track its files and sync as usual; the first `sync` creates `host/web2` on the remote.
`init --remote` on a fresh repository works too: its first push creates the branch.

## Rebuild a host

After a reinstall, or on replacement hardware, restore the old host's branch:

```bash
sudo -i
mkdir -p ~/.config/confect
cp /media/backup/age-identity.txt /media/backup/age-recipients.txt ~/.config/confect/
confect init --system --from git@git.example.com:ops/configs.git --host web1
confect restore --dry-run
confect restore --yes --backup
confect status
```

- Copy the age files first if the host has encrypted files ([Secrets and Encryption](/advanced/encryption)).
- Preview with `restore --dry-run`. `status` compares in the other direction: tracked
  directories that do not exist on the new system at all are only reported as warnings.
- `restore` sets the recorded mode and owner. Owners are matched by name, so install the
  packages that create service users (`www-data`, `postgres`, ...) before restoring.
- Continue with `sync` as before; the history continues on the same branch.

## Compare with another host

A clone made with `init --from` has the other hosts' branches as `origin/host/<name>`. Plain
git shows their files:

```bash
cd /var/lib/confect
git fetch origin
git show origin/host/web1:nginx/etc/nginx/sites-enabled/default
git diff origin/host/web1 HEAD -- nginx/etc/nginx/nginx.conf
```

Copy what you need to the system yourself; confect only restores files of its own branch.

## When the same branch moves elsewhere

Normally only one machine writes a host branch. If it was changed from another clone, bring
the local repository up to date before making changes:

```bash
confect pull             # update the repository only
confect pull --restore   # and write the pulled files to the system
```

`pull` only fast-forwards. If both sides have new commits, it stops and you resolve the
divergence with git in the repository. `sync` also refuses to overwrite the remote: a rejected
push is an error.

## Host names

The host name comes from `--host` or the hostname at `init` time and is stored in the
repository. Renaming the machine later does not switch branches. Use `--host` whenever the
hostname is not a good branch name, or when you rebuild a machine that got a different
hostname.
