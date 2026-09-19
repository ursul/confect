# info

Show repository information.

## Usage

```bash
confect info
```

## Output

```
Repository
  path     /var/lib/confect
  host     web1
  branch   host/web1
  format   2
  remote   origin git@git.example.com:ops/configs.git
  push     after every sync to 'origin'

Encryption
  recipients  /root/.config/confect/age-recipients.txt (1 key(s))
  identity    /root/.config/confect/age-identity.txt (present)

Categories
  base             7 stored
  nginx            9 stored, 2 encrypted
  total            16
```

- `push` shows `manual (auto_push = false)` when automatic pushing is off.
- `recipients` and `identity` show `missing` when the files do not exist.
- The counts are stored paths, directories included.

`info` also works on a 1.x repository. It then shows only the first block, with
`format 1 (run 'confect migrate' to convert to 2)`.
