#!/usr/bin/env bash
# Migration creation helper script

set -e

MIGRATIONS_DIR="migrations"
MIGRATION_NAME="$1"

if [ -z "$MIGRATION_NAME" ]; then
    echo "Error: Migration name required"
    echo "Usage: ./create.sh <migration_name>"
    exit 1
fi

# Validate snake_case
if [[ ! "$MIGRATION_NAME" =~ ^[a-z][a-z0-9_]*$ ]]; then
    echo "Error: Migration name must be snake_case"
    exit 1
fi

# Check for existing migration
if ls "$MIGRATIONS_DIR"/*"_$MIGRATION_NAME.sql" 2>/dev/null; then
    echo "Error: Migration with name '$MIGRATION_NAME' already exists"
    exit 1
fi

# Get next migration number
LAST_NUM=$(ls "$MIGRATIONS_DIR"/*.sql 2>/dev/null | sed 's/.*\///' | sed 's/_.*//' | sort -n | tail -1)
NEXT_NUM=$(printf "%03d" $((10#${LAST_NUM:-0} + 1)))

# Create migration file
MIGRATION_FILE="$MIGRATIONS_DIR/${NEXT_NUM}_${MIGRATION_NAME}.sql"

cat > "$MIGRATION_FILE" << EOF
-- Migration: ${MIGRATION_NAME}
-- Created at: $(date -u +"%Y-%m-%d %H:%M:%S")

-- UP: Write your migration here


-- DOWN: Write your rollback here

EOF

echo "✓ Created migration: $MIGRATION_FILE"
echo "  Edit this file to add your migration logic."
