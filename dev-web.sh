#!/bin/bash

echo "Starting Vue development server..."
echo "The Rust backend should be running separately with: cargo run -- client --web-port 8080"
echo ""
echo "Note: The vite dev server proxies API requests to http://127.0.0.1:8080"
echo ""
cd web-ui
npm run dev
