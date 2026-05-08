#!/usr/bin/env bash
set -euo pipefail

# Ensure script is run from src-tauri directory
if [ "$(basename "$PWD")" != "src-tauri" ]; then
  echo "Error: This script must be run from a directory \"src-tauri\""
  exit 1
fi

DIR="${1:-./certs}"
mkdir -p "$DIR"

KEY="$DIR/rootCA.key"
CSR="$DIR/rootCA.csr"
CRT="$DIR/rootCA.crt"

# Generate EC private key (prime256v1)
openssl ecparam -name prime256v1 -genkey -noout -out "$KEY"

# Create CSR
openssl req -new -sha256 \
  -key "$KEY" \
  -out "$CSR" \
  -subj "/C=MO/O=Elfennani/CN=localhost"

# Self-signed root CA certificate
openssl x509 -req -sha256 \
  -in "$CSR" \
  -signkey "$KEY" \
  -days 365 \
  -out "$CRT"

echo "Generated:"
echo "  $KEY"
echo "  $CSR"
echo "  $CRT"