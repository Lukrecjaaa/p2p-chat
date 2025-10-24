#!/bin/bash
set -e

echo "Building web UI..."
cd web-ui
npm install
npm run build
cd ..

echo "Building Rust binary..."
cargo build --release

echo "Done! Binary available at: target/release/p2p-chat"
