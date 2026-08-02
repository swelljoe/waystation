#!/usr/bin/env python3
"""Run the built web game in headless Chromium and look at it.

The native binary opens a real window, which a Wayland session will not let a
screenshot tool reach; the WebAssembly build has no such problem, because the
frames come back through the debugging protocol rather than the compositor. So
this is how the game gets *looked at* as well as exercised: it serves `dist`,
starts the game the way a player does, optionally walks the Scribe somewhere,
writes PNGs, and fails if the page reported anything a player would care about.

    python3 scripts/web_smoke.py --out /tmp/shots
    python3 scripts/web_smoke.py --out /tmp/shots --walk d:2.2 s:0.35 r:0.08

Requires `chromium-browser` and the `websockets` module. Build `dist` first with
`make web`, or point `--dist` at a bundle built elsewhere.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import functools
import http.server
import json
import socket
import subprocess
import sys
import threading
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Keys the game binds. `hold` presses and releases, so a short hold is a tap.
KEYS = {
    "w": ("KeyW", 87),
    "a": ("KeyA", 65),
    "s": ("KeyS", 83),
    "d": ("KeyD", 68),
    "e": ("KeyE", 69),
    "r": ("KeyR", 82),
    "q": ("KeyQ", 81),
    "p": ("KeyP", 80),
    "f4": ("F4", 115),
    "tab": ("Tab", 9),
    "space": ("Space", 32),
    "escape": ("Escape", 27),
    "left": ("ArrowLeft", 37),
    "right": ("ArrowRight", 39),
}

# Where the game keeps its save. Seeding it is the only way to look at a screen
# that takes a night's play to reach.
SAVE_KEY = "waystation-save-v1"

# Chromium has no GPU here, and Bevy needs a real WebGL2 context, so ANGLE is
# pointed at its software rasteriser.
SOFTWARE_WEBGL = [
    "--use-gl=angle",
    "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader",
]


def is_expected(message: str) -> bool:
    """True for page complaints that are normal and not worth failing over.

    Bevy asks for a `.meta` beside every asset and ships none of them, and a
    browser will not start an `AudioContext` until the player has clicked, which
    is exactly what the title screen is for.
    """
    return (
        ".meta" in message
        or "favicon" in message
        or "AudioContext was not allowed to start" in message
        # Chromium ignores `integrity` on the preload links trunk emits and says
        # so on every load; crbug.com/981419.
        or "`integrity` attribute is currently ignored" in message
    )


def free_port() -> int:
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


# The listening call the game makes when a visitor finishes their story. A
# static file server answers POST with 501, which the game correctly treats as a
# dead service and falls back from — but that turns a working visit into a
# console error and hides real faults behind it. Answering here exercises the
# same path a live Gloo/YouVersion deployment takes, with provenance that says
# plainly where it came from.
STUB_INTERPRETATION = {
    "vignette_id": "",
    "need_id": "belonging",
    "need_label": "A place of equal dignity",
    "reflection": "No living soul is extra. A name spoken with welcome becomes part of the shelter.",
    "passage": {
        "id": "GAL.3.28",
        "reference": "Galatians 3:28",
        "content": (
            "There is neither Jew nor Greek, slave nor free, male nor female, "
            "for you are all one in Christ Jesus."
        ),
        "version": "BSB",
        "youversion_deep_link": "https://www.bible.com/bible/3034/GAL.3.28",
    },
    "provenance": {
        "gloo_model": "web-smoke-stub",
        "routing": "web-smoke",
        "scripture_source": "fixture",
    },
}


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    """The asset `.meta` probes would otherwise bury the run in 404 lines."""

    def log_message(self, *_args) -> None:
        pass

    def do_POST(self) -> None:  # noqa: N802 - the base class names it this
        if self.path != "/api/interpret":
            self.send_error(404)
            return
        length = int(self.headers.get("Content-Length") or 0)
        request = json.loads(self.rfile.read(length) or b"{}")
        reply = dict(STUB_INTERPRETATION, vignette_id=request.get("vignette_id", ""))
        body = json.dumps(reply).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def serve(directory: Path) -> tuple[http.server.ThreadingHTTPServer, int]:
    port = free_port()
    handler = functools.partial(QuietHandler, directory=str(directory))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, port


class Page:
    """The few CDP calls this needs, without a browser-automation dependency."""

    def __init__(self, socket_, out: Path) -> None:
        self.socket = socket_
        self.out = out
        self.next_id = 0
        self.problems: list[str] = []

    async def call(self, method: str, **params):
        self.next_id += 1
        message_id = self.next_id
        await self.socket.send(json.dumps({"id": message_id, "method": method, "params": params}))
        while True:
            reply = json.loads(await self.socket.recv())
            if reply.get("id") == message_id:
                if "error" in reply:
                    raise RuntimeError(f"{method}: {reply['error']}")
                return reply.get("result", {})
            self.record(reply)

    def record(self, event: dict) -> None:
        if event.get("method") == "Log.entryAdded":
            entry = event["params"]["entry"]
            text = f"{entry.get('text', '')} {entry.get('url', '')}"
            if entry["level"] in {"error", "warning"} and not is_expected(text):
                self.problems.append(f"{entry['level']}: {text.strip()}")
        elif event.get("method") == "Runtime.exceptionThrown":
            details = event["params"]["exceptionDetails"]
            self.problems.append(f"exception: {details.get('text', '')} {json.dumps(details)[:300]}")

    async def drain(self, seconds: float = 0.2) -> None:
        while True:
            try:
                self.record(json.loads(await asyncio.wait_for(self.socket.recv(), timeout=seconds)))
            except (asyncio.TimeoutError, Exception):  # noqa: B014 - closed socket ends the drain
                return

    async def evaluate(self, expression: str):
        result = await self.call("Runtime.evaluate", expression=expression, returnByValue=True)
        return result.get("result", {}).get("value")

    async def until(self, expression: str, label: str, attempts: int = 240) -> None:
        for _ in range(attempts):
            await self.drain()
            if await self.evaluate(expression):
                return
            await asyncio.sleep(0.5)
        raise SystemExit(f"timed out waiting for {label}\n  " + "\n  ".join(self.problems))

    async def hold(self, name: str, seconds: float) -> None:
        code, key_code = KEYS[name]
        for down in (True, False):
            await self.call(
                "Input.dispatchKeyEvent",
                type="keyDown" if down else "keyUp",
                code=code,
                key=name,
                windowsVirtualKeyCode=key_code,
                nativeVirtualKeyCode=key_code,
            )
            if down:
                await asyncio.sleep(seconds)

    async def shot(self, name: str) -> Path:
        result = await self.call("Page.captureScreenshot", format="png")
        path = self.out / f"{name}.png"
        path.write_bytes(base64.b64decode(result["data"]))
        print(f"wrote {path}")
        return path


def parse_headers(pairs: list[str]) -> dict[str, str]:
    """`Name: value` strings, as the command line spells them, into a CDP map."""
    headers = {}
    for pair in pairs:
        name, separator, value = pair.partition(":")
        if not separator or not name.strip():
            raise SystemExit(f"--header wants `Name: value`, not {pair!r}")
        headers[name.strip()] = value.strip()
    return headers


def page_url(origin: int | str, query: str = "") -> str:
    """The page to open, with any query string the caller asked for.

    The game reads `?visitors=` at startup to stand a visit up immediately, so a
    screenshot of a traveller does not have to wait out three nights of smoke.

    A bare port means the throwaway static server below. A whole origin means
    somewhere else entirely — the real server, which is the only way to exercise
    `/api/interpret`, or a credentialed one, which is the only way to find out
    whether the asset loader's own fetches carry the password the page was opened
    with. Both are things the static server cannot answer for.
    """
    if isinstance(origin, int):
        origin = f"http://127.0.0.1:{origin}"
    address = f"{origin.rstrip('/')}/index.html"
    query = query.lstrip("?")
    return f"{address}?{query}" if query else address


def attach(port: int, seconds: float = 30.0) -> str:
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=2) as reply:
                pages = [t for t in json.load(reply) if t["type"] == "page"]
            if pages:
                return pages[0]["webSocketDebuggerUrl"]
        except Exception:
            pass
        time.sleep(0.5)
    raise SystemExit("chromium never exposed a page target")


async def run(args) -> int:
    import websockets

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    server = None
    if args.origin:
        origin: int | str = args.origin
    else:
        server, http_port = serve(Path(args.dist))
        origin = http_port
    # Credentials cannot simply be written into the address: `fetch` refuses any
    # URL carrying them, so the page would load and the wasm beside it would not.
    # A browser sends them as a header once a person has answered the dialog, and
    # extra headers are how that is reproduced — which means waiting for the
    # debugger before navigating anywhere.
    target = page_url(origin, args.query)
    debug_port = free_port()
    chromium = subprocess.Popen(
        [
            args.browser,
            "--headless=new",
            f"--remote-debugging-port={debug_port}",
            "--remote-allow-origins=*",
            f"--window-size={args.view}",
            "--hide-scrollbars",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-gpu-sandbox",
            *SOFTWARE_WEBGL,
            f"--user-data-dir={out / 'chromium-profile'}",
            "about:blank" if args.header else target,
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        endpoint = attach(debug_port)
        async with websockets.connect(endpoint, max_size=64 * 1024 * 1024) as socket_:
            page = Page(socket_, out)
            for domain in ("Page", "Runtime", "Log"):
                await page.call(f"{domain}.enable")
            if args.header:
                await page.call("Network.enable")
                await page.call("Network.setExtraHTTPHeaders", headers=parse_headers(args.header))
                await page.call("Page.navigate", url=target)

            await page.until(
                "!!(window.wasmBindings && window.wasmBindings.start_web_game)",
                "the wasm bundle",
            )
            await page.shot("00-title")

            # Before the click, because the game reads its save at startup.
            if args.save:
                save = Path(args.save).read_text(encoding="utf-8")
                await page.evaluate(
                    f"localStorage.setItem({json.dumps(SAVE_KEY)}, {json.dumps(save)})"
                )

            # A real click, so the wasm entry point and the audio gesture take
            # the same path they take for a player.
            await page.evaluate("document.querySelector('.start-game').click()")
            await page.until("!!document.querySelector('canvas')", "the game canvas", attempts=120)
            await asyncio.sleep(args.settle)
            await page.shot("01-start")

            for index, step in enumerate(args.walk, start=2):
                key, _, seconds = step.partition(":")
                if key not in KEYS:
                    raise SystemExit(f"unknown key in --walk: {key}")
                await page.hold(key, float(seconds or 0.1))
                await asyncio.sleep(0.4)
                await page.shot(f"{index:02d}-{key}")

            await page.drain(1.0)
            if page.problems:
                print("page reported:", *dict.fromkeys(page.problems), sep="\n  ")
                return 1
            print("no unexpected console errors")
            return 0
    finally:
        chromium.terminate()
        if server is not None:
            server.shutdown()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", default=str(ROOT / "dist"), help="built web bundle to serve")
    parser.add_argument("--out", default=str(ROOT / "target/web-smoke"), help="screenshot directory")
    parser.add_argument("--browser", default="chromium-browser")
    parser.add_argument(
        "--save",
        help="JSON save file to put in local storage before the game starts, so a"
        " screen that takes nights of play to reach can still be looked at",
    )
    parser.add_argument(
        "--query",
        default="",
        help="query string for index.html, e.g. visitors=repeat to stand a visit up at once",
    )
    parser.add_argument(
        "--view",
        default="1920,1080",
        help="browser window size; the camera scale is fixed, so a wider window shows more world",
    )
    parser.add_argument(
        "--settle", type=float, default=6.0, help="seconds to let the world load before the shot"
    )
    parser.add_argument(
        "--walk",
        nargs="*",
        default=[],
        help="key:seconds steps to run after starting, e.g. d:2.2 s:0.35 r:0.08",
    )
    parser.add_argument(
        "--origin",
        help="serve nothing and point the browser at this origin instead, e.g."
        " http://127.0.0.1:7788 for the real server rather than a static directory",
    )
    parser.add_argument(
        "--header",
        action="append",
        default=[],
        metavar="NAME: VALUE",
        help="extra header on every request; `Authorization: Basic ...` is how a"
        " gated build is exercised, since credentials in the URL break fetch",
    )
    args = parser.parse_args()
    if not args.origin and not Path(args.dist, "index.html").is_file():
        raise SystemExit(f"no web bundle at {args.dist}; run `make web` first")
    return asyncio.run(run(args))


if __name__ == "__main__":
    sys.exit(main())
