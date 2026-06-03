lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run the game.
game:
    cargo run -p arenic

# Run the storybook / design-system harness.
storybook:
    cargo run -p arenic_storybook

# Regenerate the Lucide icon PNGs (needs curl + rsvg-convert).
icons:
    ./scripts/gen-icons.sh

# Build the docs + design-system book into ./book.
book:
    mdbook build

# Serve the book locally with live reload.
book-serve:
    mdbook serve --open

gate: lint book
