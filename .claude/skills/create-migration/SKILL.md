---
name: create-migration
description: Generate SQLx database migration with naming validation and template
disable-model-invocation: true
---

# Create Database Migration

Generate a new SQLx migration file for the Astral Key project following project conventions.

## Usage

Invoke this skill when you need to create a new database migration. Provide:
- **Migration name**: Descriptive name in snake_case (e.g., `add_user_last_login`)
- **Changes**: Description of table/column changes needed

## What This Skill Does

1. Validates the migration name follows snake_case convention
2. Checks for existing migrations with the same name
3. Creates a new migration file with auto-incremented number
4. Generates UP and DOWN migration sections
5. Follows Astral Key conventions (UUID primary keys, timestamps, etc.)

## Astral Key Migration Conventions

- **Primary keys**: Always use `UUID` with `DEFAULT gen_random_uuid()`
- **Timestamps**: Include `created_at` and `updated_at` columns
- **Foreign keys**: Reference UUID columns with `ON DELETE CASCADE`
- **Indexes**: Add indexes for frequently queried columns
- **Naming**: Table names are plural, columns are snake_case

## Example Migration File

```sql
-- Migration: Add user last login
-- Created at: 2024-01-15 10:30:00

-- UP
ALTER TABLE users ADD COLUMN last_login_at TIMESTAMPTZ;
CREATE INDEX idx_users_last_login ON users(last_login_at);

-- DOWN
DROP INDEX IF EXISTS idx_users_last_login;
ALTER TABLE users DROP COLUMN IF EXISTS last_login_at;
```

## Common Patterns

### Add a new table
```sql
CREATE TABLE table_name (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

### Add a foreign key
```sql
ALTER TABLE child_table
ADD COLUMN parent_id UUID REFERENCES parent_table(id) ON DELETE CASCADE;
CREATE INDEX idx_child_table_parent ON child_table(parent_id);
```

### Add a column with default
```sql
ALTER TABLE users ADD COLUMN is_verified BOOLEAN DEFAULT FALSE;
```

## Output

Returns the path to the created migration file and confirms successful creation.
