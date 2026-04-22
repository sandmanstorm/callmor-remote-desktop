#!/usr/bin/env bash
# Grant/revoke platform super-admin to a user.
# Usage: ./scripts/grant-superadmin.sh <email> [true|false]
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: $0 <email> [true|false]"
    echo "Example: $0 admin@example.com true"
    exit 1
fi

EMAIL="$1"
VALUE="${2:-true}"

if [ "$VALUE" != "true" ] && [ "$VALUE" != "false" ]; then
    echo "Second argument must be 'true' or 'false'"
    exit 1
fi

# Use sg docker so we don't need the user to be in the docker group interactively
CONTAINER="callmor-remote-desktop-postgres-1"
sg docker -c "docker exec -i $CONTAINER psql -U callmor -d callmor -c \"UPDATE users SET is_superadmin = $VALUE WHERE email = '$EMAIL' RETURNING email, display_name, is_superadmin;\""
