# category

Manage file categories.

## Usage

```bash
confect category <COMMAND>
```

## Commands

| Command | Description |
|---------|-------------|
| `list` | List all categories |
| `create <NAME>` | Create a new category |
| `delete <NAME>` | Delete a category |
| `add <CATEGORY> <PATH>` | Add file to category |
| `remove <CATEGORY> <PATH>` | Remove file from category |

## Examples

```bash
# List categories
confect category list

# Create category
confect category create shell

# Add file to category
confect category add shell ~/.zshrc

# List files in category
confect category list shell
```
