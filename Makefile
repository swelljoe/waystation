.PHONY: assets prints add-print print-art catalog wardrobe npcs bible-versions verses editor check test analyze build server server-live game web web-smoke publish-demo-assets container run-container

assets:
	python3 scripts/build-assets.py
	python3 scripts/build-npc-art.py $(if $(LPC),--lpc $(LPC))

prints:
	python3 scripts/build-print-cards.py

add-print:
	python3 scripts/add-print.py

print-art:
	python3 scripts/generate-print-art.py

catalog:
	python3 scripts/asset_catalog.py --output assets/.catalog.json

# Rebuilds the curated NPC wardrobe from a local Universal LPC Spritesheet
# Generator checkout. Only needed after editing the allowlist in the script or
# pulling new LPC art; the result is committed and compiled into the game.
wardrobe:
	python3 scripts/build-npc-wardrobe.py $(if $(LPC),--lpc $(LPC))

# A cast of generated travellers, as loadable LPC character files, a page of
# links into the web generator, and a contact sheet to look at. Pass ERA=dyed
# for the later, brighter palette, or COUNT= for more of them.
#
# ART=attribution-only refuses share-alike art, for travellers going into a
# screenshot, trailer, or store art — anywhere they are flattened into one image
# with purchased tilesets that cannot be relicensed to match.
npcs:
	cargo run -p waystation-npcgen --bin npc-preview -- \
		--count $(or $(COUNT),24) --era $(or $(ERA),scavenged) \
		--art $(or $(ART),any)
	python3 scripts/preview-npcs.py $(if $(LPC),--lpc $(LPC))

# Rebuilds content/bible-versions.json from the YouVersion catalog. Needs
# YVP_APP_KEY. The result is committed and reviewed; the server never discovers
# translations at runtime.
bible-versions:
	python3 scripts/fetch-bible-versions.py

# Refetches every verse in content/prints.json and content/readings.json from
# YouVersion. Needs YVP_APP_KEY. Pass VERSION= to change translation.
verses:
	python3 scripts/fetch-verses.py $(if $(VERSION),--version $(VERSION))

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
	python3 scripts/test_fetch_bible_versions.py
	python3 scripts/test_fetch_verses.py
	python3 scripts/test_publish_demo_assets.py
	python3 scripts/test_build_npc_wardrobe.py
	python3 scripts/test_build_npc_art.py
	python3 scripts/test_preview_npcs.py
	python3 scripts/test_lpc_art_tools.py
	python3 scripts/test_web_smoke.py
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

# Real Gloo routing against the authored vignettes. Credentials come from a
# gitignored .env, never from the source tree; copy .env.example and fill it in.
# YouVersion is optional here — without a key the reviewed wording in
# content/passages.ron is served and labelled as such.
server-live:
	@test -f .env || { echo "make server-live needs a .env; start from .env.example"; exit 1; }
	set -a && . ./.env && set +a && API_MODE=live cargo run -p waystation-server

game:
	cargo run -p waystation-game

web: assets
	NO_COLOR=true trunk build --release

# Looks at the game as well as exercising it: a Wayland session will not let a
# screenshot tool reach the native window, but the WebAssembly build hands back
# frames through the debugging protocol. Needs `make web` and chromium.
web-smoke:
	python3 scripts/web_smoke.py --out target/web-smoke $(WALK)

publish-demo-assets:
	python3 scripts/publish-demo-assets.py

container: assets
	podman build -t waystation:latest -f Containerfile .

run-container:
	podman run --rm -p 7777:7777 --env API_MODE=fixture waystation:latest
