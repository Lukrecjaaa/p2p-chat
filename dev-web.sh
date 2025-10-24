#!/bin/bash

echo "Starting Vue development server..."
echo "The Rust backend should be running separately with: cargo run -- client"
echo ""
cd web-ui
npm run dev
