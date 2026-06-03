# Thin verb layer. Real logic lives in `xtask` (Rust) or the tools below
# (cargo, trunk, mdbook) — never in shell scripts.

# Run the game natively.
game:
    cargo run -p arenic

# Run the storybook / design-system harness natively.
storybook:
    cargo run -p arenic_storybook

# Regenerate the Lucide icon PNGs (pure-Rust; no librsvg/curl).
icons:
    cargo xtask icons

# Build the docs + design-system book into ./book.
book:
    mdbook build

# Serve the book locally with live reload.
book-serve:
    mdbook serve --open

# Build the full web bundle (game → /app, storybook → /storybook, book → /book)
# into ./dist, mirroring the GitHub Pages layout. Needs trunk + mdbook. Trunk runs
# from each crate dir (it reads `cargo metadata` from CWD; the workspace root is
# virtual, so running there fails to find the target package).
web:
    cd crates/arenic           && trunk build --release --public-url /app/       --dist ../../dist/app       index.html
    cd crates/arenic_storybook && trunk build --release --public-url /storybook/ --dist ../../dist/storybook index.html
    MDBOOK_OUTPUT__HTML__SITE_URL=/book/ mdbook build -d dist/book
    cp web/index.html dist/index.html
    @echo "Preview: python3 -m http.server -d dist 8080  →  http://localhost:8080/"

# Serve the game locally with live reload (run from its crate dir).
dev:
    cd crates/arenic && trunk serve --public-url / index.html

# Lint the shipped crates (xtask, a dev tool, is excluded).
lint:
    cargo clippy --workspace --exclude xtask --all-targets --all-features -- -D warnings

gate: lint book
