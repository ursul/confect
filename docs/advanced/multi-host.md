# Multi-host Setup

confect supports managing configurations across multiple machines using per-host branches.

## How it works

Each host gets its own branch:

```
main                  # Shared base configs
├── host/laptop       # Laptop-specific
├── host/desktop      # Desktop-specific
└── host/server       # Server-specific
```

## Setup

### First machine

```bash
confect init --remote git@github.com:user/configs.git
confect add ~/.bashrc
confect sync
```

### Additional machines

```bash
# Initialize (creates new host branch)
confect init --remote git@github.com:user/configs.git

# Pull shared configs from main
confect pull

# Add machine-specific configs
confect add ~/.config/machine-specific
confect sync
```

## Branch structure

confect automatically:
- Creates a `host/<hostname>` branch on init
- Checks out the host branch for your machine
- Commits changes to the host branch

## Sharing configs between hosts

Common configs can be stored on `main` and merged into host branches:

```bash
git checkout main
# Add shared configs
git checkout host/laptop
git merge main
```

## Host detection

confect uses the system hostname by default. Override with:

```toml
# ~/.config/confect/config.toml
[hosts]
current = "my-custom-name"
```
