.PHONY: assets check test analyze build server game web container run-container

assets:
	python3 scripts/build-assets.py

check:
	cargo check --workspace

test:
	cargo test --workspace

analyze:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings

build: assets
	cargo build --workspace

server:
	API_MODE=fixture cargo run -p waystation-server

game:
	cargo run -p waystation-game

web: assets
	trunk build web/index.html --release --dist dist

container: assets
	podman build -t waystation:latest -f Containerfile .

run-container:
	podman run --rm -p 7777:7777 --env API_MODE=fixture waystation:latest
