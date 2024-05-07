START_DIR=$(pwd)

echo "🛠️ Generating schema...!"
CMD="cargo run --bin schema"

# discard output
eval $CMD > /dev/null

# remove redundant schemas
rm -rf ./schema/raw
echo "✅ Schemas generated."
