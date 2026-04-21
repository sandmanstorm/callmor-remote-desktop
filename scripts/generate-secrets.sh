#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$REPO_ROOT/.env"
KEYS_DIR="$REPO_ROOT/keys"

if [ -f "$ENV_FILE" ]; then
    echo "⚠  .env already exists. Remove it first if you want to regenerate."
    exit 1
fi

echo "Generating secrets..."

# Generate random 32-char alphanumeric passwords
gen_pass() {
    openssl rand -base64 32 | tr -dc 'a-zA-Z0-9' | head -c 32
}

PG_PASS=$(gen_pass)
REDIS_PASS=$(gen_pass)
MINIO_PASS=$(gen_pass)
TURN_SECRET=$(gen_pass)

# Generate JWT RS256 keypair
mkdir -p "$KEYS_DIR"
openssl genpkey -algorithm RSA -out "$KEYS_DIR/jwt_private.pem" -pkeyopt rsa_keygen_bits:2048 2>/dev/null
openssl rsa -pubout -in "$KEYS_DIR/jwt_private.pem" -out "$KEYS_DIR/jwt_public.pem" 2>/dev/null
chmod 600 "$KEYS_DIR/jwt_private.pem"
chmod 644 "$KEYS_DIR/jwt_public.pem"

cat > "$ENV_FILE" <<EOF
# Postgres
POSTGRES_USER=callmor
POSTGRES_PASSWORD=${PG_PASS}
POSTGRES_DB=callmor
DATABASE_URL=postgres://callmor:${PG_PASS}@127.0.0.1:5432/callmor

# Redis
REDIS_PASSWORD=${REDIS_PASS}
REDIS_URL=redis://:${REDIS_PASS}@127.0.0.1:6379

# MinIO
MINIO_ROOT_USER=callmor
MINIO_ROOT_PASSWORD=${MINIO_PASS}
MINIO_ENDPOINT=http://127.0.0.1:9000

# coturn
TURN_SECRET=${TURN_SECRET}
TURN_REALM=callmor.ai

# JWT (RS256 keypair paths)
JWT_PRIVATE_KEY_PATH=./keys/jwt_private.pem
JWT_PUBLIC_KEY_PATH=./keys/jwt_public.pem

# Server ports
RELAY_PORT=8080
API_PORT=3000
EOF

chmod 600 "$ENV_FILE"

echo "Done. Generated:"
echo "  $ENV_FILE (chmod 600)"
echo "  $KEYS_DIR/jwt_private.pem (chmod 600)"
echo "  $KEYS_DIR/jwt_public.pem (chmod 644)"
