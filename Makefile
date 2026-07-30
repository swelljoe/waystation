.PHONY: assets prints add-print print-art catalog editor check test analyze build server game web publish-demo-assets container run-container

assets:
	python3 scripts/build-assets.py

prints:
	python3 scripts/build-print-cards.py

add-print:
	python3 scripts/add-print.py

print-art:
	python3 scripts/generate-print-art.py

catalog:
	python3 scripts/asset_catalog.py --output assets/.catalog.json

editor:
	python3 scripts/level_editor.py

check:
	cargo check --workspace

test:
	python3 scripts/test_asset_catalog.py
	python3 scripts/test_build_assets.py
	python3 scripts/test_build_print_cards.py
	python3 scripts/test_add_print.py
	python3 scripts/test_generate_print_art.py
	python3 scripts/test_publish_demo_assets.py
	python3 scripts/test_level_editor.py
	node scripts/test_level_editor_ui.js
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
	NO_COLOR=true trunk build --release

publish-demo-assets:
	python3 scripts/publish-demo-assets.py

container: assets
	podman build -t waystation:latest -f Containerfile .

run-container:
	podman run --rm -p 7777:7777 --env API_MODE=fixture waystation:latest
