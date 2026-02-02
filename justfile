# Build release binary and install to /usr/local/bin
release:
    cargo build --release
    sudo cp target/release/notes /usr/local/bin/
