const $ = (selector) => document.querySelector(selector);
const layerOrder = { floor: 0, wall: 1, object: 2, overlay: 3 };
const imageCache = new Map();

const state = {
  catalog: null,
  filtered: [],
  sheet: null,
  sheetImage: null,
  sheetDrag: null,
  selection: null,
  tool: "stamp",
  layer: "floor",
  zoom: 1,
  room: null,
  undo: [],
  stateSources: { damaged: null, repaired: null },
};

function freshRoom() {
  return {
    schema_version: 2,
    id: "motel-room-01",
    name: "Motel Room 01",
    background: "#49382b",
    floor_line: "#34261d",
    grid: { width: 18, height: 11, tile_size: 32 },
    entry: { x: 8, y: 9 },
    exits: [{ x: 8, y: 10, to: "exterior", spawn: "motel-door" }],
    collision: [],
    placements: [],
    templates: {},
    structures: [],
    fixtures: [],
  };
}

function normalizeRoom(room) {
  room.schema_version = 2;
  room.placements ||= [];
  room.templates ||= {};
  room.structures ||= [];
  room.fixtures ||= [];
  return room;
}

function assetUrl(path) {
  return `/asset/${path.split("/").map(encodeURIComponent).join("/")}`;
}

function getImage(path) {
  if (!imageCache.has(path)) {
    const image = new Image();
    image.decoding = "async";
    image.src = assetUrl(path);
    image.addEventListener("load", drawRoom);
    imageCache.set(path, image);
  }
  return imageCache.get(path);
}

function setStatus(message) { $("#status").textContent = message; }

function syncInputs() {
  $("#room-id").value = state.room.id;
  $("#room-name").value = state.room.name;
  $("#room-width").value = state.room.grid.width;
  $("#room-height").value = state.room.grid.height;
  $("#background").value = state.room.background;
}

function pushUndo() {
  state.undo.push(JSON.stringify(state.room));
  if (state.undo.length > 80) state.undo.shift();
}

function undo() {
  const previous = state.undo.pop();
  if (!previous) return;
  state.room = JSON.parse(previous);
  syncInputs();
  drawRoom();
  setStatus("Undid the last edit.");
}

function roomTileSize() {
  return state.room.grid.tile_size;
}

function placementPixelSize(placement) {
  if (placement.repeat) {
    return {
      width: placement.width * roomTileSize(),
      height: placement.height * roomTileSize(),
    };
  }
  return {
    width: placement.source.width * placement.source.grid,
    height: placement.source.height * placement.source.grid,
  };
}

function roomRenderables() {
  const renderables = state.room.placements.map((placement, index) => ({
    placement,
    collection: "placements",
    index,
  }));
  for (const collection of ["structures", "fixtures"]) {
    state.room[collection].forEach((element, index) => {
      const template = state.room.templates[element.template];
      const visual = template?.states[element.initial_state];
      if (!visual || visual.visible === false || !visual.source) return;
      renderables.push({
        placement: { ...element, layer: template.layer, source: visual.source },
        collection,
        index,
      });
    });
  }
  return renderables.sort((a, b) => layerOrder[a.placement.layer] - layerOrder[b.placement.layer]);
}

function roomCell(event) {
  const canvas = $("#room-canvas");
  const rect = canvas.getBoundingClientRect();
  const tile = roomTileSize() * state.zoom;
  const x = Math.floor(((event.clientX - rect.left) * canvas.width / rect.width) / tile);
  const y = Math.floor(((event.clientY - rect.top) * canvas.height / rect.height) / tile);
  if (x < 0 || y < 0 || x >= state.room.grid.width || y >= state.room.grid.height) return null;
  return { x, y };
}

function drawRoom() {
  const canvas = $("#room-canvas");
  const tile = roomTileSize() * state.zoom;
  canvas.width = Math.round(state.room.grid.width * tile);
  canvas.height = Math.round(state.room.grid.height * tile);
  const context = canvas.getContext("2d");
  context.imageSmoothingEnabled = false;
  context.fillStyle = state.room.background;
  context.fillRect(0, 0, canvas.width, canvas.height);

  for (const { placement } of roomRenderables()) {
    const image = getImage(placement.source.path);
    if (!image.complete || !image.naturalWidth) continue;
    const source = placement.source;
    const sourceWidth = source.width * source.grid;
    const sourceHeight = source.height * source.grid;
    const destinationX = placement.x * tile;
    const destinationY = placement.y * tile;
    if (placement.repeat) {
      const repeatWidth = placement.width * tile;
      const repeatHeight = placement.height * tile;
      context.save();
      context.beginPath();
      context.rect(destinationX, destinationY, repeatWidth, repeatHeight);
      context.clip();
      for (let y = 0; y < repeatHeight; y += sourceHeight * state.zoom) {
        for (let x = 0; x < repeatWidth; x += sourceWidth * state.zoom) {
          context.drawImage(
            image,
            source.x * source.grid,
            source.y * source.grid,
            sourceWidth,
            sourceHeight,
            destinationX + x,
            destinationY + y,
            sourceWidth * state.zoom,
            sourceHeight * state.zoom,
          );
        }
      }
      context.restore();
    } else {
      context.drawImage(
        image,
        source.x * source.grid,
        source.y * source.grid,
        sourceWidth,
        sourceHeight,
        destinationX,
        destinationY,
        sourceWidth * state.zoom,
        sourceHeight * state.zoom,
      );
    }
  }

  context.lineWidth = 1;
  context.strokeStyle = "rgba(224,196,134,.18)";
  context.beginPath();
  for (let x = 0; x <= state.room.grid.width; x++) {
    context.moveTo(Math.round(x * tile) + .5, 0);
    context.lineTo(Math.round(x * tile) + .5, canvas.height);
  }
  for (let y = 0; y <= state.room.grid.height; y++) {
    context.moveTo(0, Math.round(y * tile) + .5);
    context.lineTo(canvas.width, Math.round(y * tile) + .5);
  }
  context.stroke();

  context.fillStyle = "rgba(180,65,65,.42)";
  for (const cell of state.room.collision) context.fillRect(cell.x * tile, cell.y * tile, tile, tile);
  context.font = `${Math.max(12, tile * .48)}px system-ui`;
  context.textAlign = "center";
  context.textBaseline = "middle";
  context.fillStyle = "#e5c66e";
  context.fillText("E", (state.room.entry.x + .5) * tile, (state.room.entry.y + .5) * tile);
  context.fillStyle = "#7ed0b0";
  for (const exit of state.room.exits) context.fillText("⇩", (exit.x + .5) * tile, (exit.y + .5) * tile);
}

function eraseAt(cell) {
  const tileSize = roomTileSize();
  const cellLeft = cell.x * tileSize;
  const cellTop = cell.y * tileSize;
  const renderables = roomRenderables();
  for (let renderIndex = renderables.length - 1; renderIndex >= 0; renderIndex--) {
    const { placement: p, collection, index } = renderables[renderIndex];
    const size = placementPixelSize(p);
    const placementLeft = p.x * tileSize;
    const placementTop = p.y * tileSize;
    if (
      cellLeft < placementLeft + size.width &&
      cellLeft + tileSize > placementLeft &&
      cellTop < placementTop + size.height &&
      cellTop + tileSize > placementTop
    ) {
      state.room[collection].splice(index, 1);
      return true;
    }
  }
  return false;
}

function editRoom(cell, forceErase = false) {
  if (!cell) return;
  const tool = forceErase ? "erase" : state.tool;
  if (tool === "stamp") {
    const behavior = $("#behavior").value;
    if (behavior === "baked" && !state.selection) { setStatus("Select a source-sheet rectangle first."); return; }
    pushUndo();
    const tileSize = roomTileSize();
    if (behavior === "baked") {
      const s = state.selection;
      state.room.placements.push({
        layer: state.layer,
        x: cell.x,
        y: cell.y,
        width: Math.ceil(s.width * s.grid / tileSize),
        height: Math.ceil(s.height * s.grid / tileSize),
        source: selectionSource(),
      });
      setStatus(`Placed ${s.width * s.grid}×${s.height * s.grid}px baked stamp on ${state.layer}.`);
    } else {
      const collection = behavior === "structure" ? "structures" : "fixtures";
      const templateId = slugify($("#element-kind").value) || (behavior === "structure" ? state.layer : "fixture");
      const label = $("#element-label").value.trim() || templateId.replaceAll("-", " ");
      let template = state.room.templates[templateId];
      if (!template) {
        if (!state.stateSources.damaged || !state.stateSources.repaired) {
          state.undo.pop();
          setStatus(`Template ${templateId} is new; capture both damaged and repaired crops first.`);
          return;
        }
        template = {
          label,
          kind: templateId,
          layer: state.layer,
          states: {
            damaged: { source: structuredClone(state.stateSources.damaged) },
            repaired: { source: structuredClone(state.stateSources.repaired) },
          },
        };
        state.room.templates[templateId] = template;
      }
      const stateSizes = Object.values(template.states)
        .filter((visual) => visual.visible !== false && visual.source)
        .map((visual) => ({
          width: visual.source.width * visual.source.grid,
          height: visual.source.height * visual.source.grid,
        }));
      if (!stateSizes.length) {
        state.undo.pop();
        setStatus(`Template ${templateId} has no visible state crops.`);
        return;
      }
      const width = Math.ceil(Math.max(...stateSizes.map((size) => size.width)) / tileSize);
      const height = Math.ceil(Math.max(...stateSizes.map((size) => size.height)) / tileSize);
      const id = nextElementId(templateId);
      state.room[collection].push({
        id,
        template: templateId,
        x: cell.x,
        y: cell.y,
        width,
        height,
        initial_state: "damaged",
      });
      setStatus(`Placed repairable ${template.label} as ${id}; future stamps can reuse template ${templateId}.`);
    }
  } else if (tool === "erase") {
    pushUndo();
    if (!eraseAt(cell)) state.undo.pop();
  } else if (tool === "collision") {
    pushUndo();
    const index = state.room.collision.findIndex((item) => item.x === cell.x && item.y === cell.y);
    if (index >= 0) state.room.collision.splice(index, 1);
    else state.room.collision.push(cell);
  } else if (tool === "entry") {
    pushUndo(); state.room.entry = cell;
  } else if (tool === "exit") {
    pushUndo();
    const index = state.room.exits.findIndex((item) => item.x === cell.x && item.y === cell.y);
    if (index >= 0) state.room.exits.splice(index, 1);
    else state.room.exits.push({ ...cell, to: "exterior", spawn: "motel-door" });
  }
  drawRoom();
}

function selectionSource() {
  const s = state.selection;
  return { path: state.sheet.path, grid: s.grid, x: s.x, y: s.y, width: s.width, height: s.height };
}

function slugify(value) {
  return value.toLowerCase().trim().replace(/[^a-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 52);
}

function nextElementId(kind) {
  const used = new Set([...state.room.structures, ...state.room.fixtures].map((element) => element.id));
  for (let sequence = 1; sequence < 10000; sequence++) {
    const candidate = `${kind}-${String(sequence).padStart(2, "0")}`;
    if (!used.has(candidate)) return candidate;
  }
  throw new Error(`Could not allocate an ID for ${kind}`);
}

function captureStateSource(stateName) {
  if (!state.selection || !state.sheet) {
    setStatus("Select a source-sheet rectangle first.");
    return;
  }
  state.stateSources[stateName] = selectionSource();
  updateStateSourceDetails();
  setStatus(`Captured ${stateName} crop at ${state.selection.width * state.selection.grid}×${state.selection.height * state.selection.grid}px.`);
}

function updateStateSourceDetails() {
  for (const stateName of ["damaged", "repaired"]) {
    const source = state.stateSources[stateName];
    $(`#${stateName}-source`).textContent = source
      ? `${source.path.split("/").at(-1)} · ${source.width * source.grid}×${source.height * source.grid}px`
      : "not captured";
  }
}

function filterAssets() {
  const query = $("#asset-search").value.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const pack = $("#pack-filter").value;
  state.filtered = state.catalog.assets.filter((asset) => {
    if (pack && asset.pack !== pack) return false;
    const haystack = `${asset.path} ${asset.tags.join(" ")}`.toLowerCase();
    return query.every((word) => haystack.includes(word));
  });
  renderAssets();
}

function renderAssets() {
  const list = $("#asset-list");
  list.replaceChildren();
  const visible = state.filtered.slice(0, 160);
  for (const asset of visible) {
    const card = document.createElement("button");
    card.className = `asset-card${state.sheet?.id === asset.id ? " active" : ""}`;
    card.title = `${asset.path}\n${asset.tags.join(", ")}`;
    card.innerHTML = `<span class="asset-thumb"><img loading="lazy" src="${assetUrl(asset.path)}" alt=""></span><span class="asset-meta"><strong>${asset.name}</strong><small>${asset.pack} · ${asset.width}×${asset.height} · grid ${asset.grid}</small></span>`;
    card.addEventListener("click", () => selectSheet(asset));
    list.append(card);
  }
  $("#asset-count").textContent = `${state.filtered.length.toLocaleString()} found${state.filtered.length > visible.length ? ` · showing ${visible.length}` : ""}`;
}

function selectSheet(asset) {
  state.sheet = asset;
  state.selection = null;
  $("#source-grid").value = asset.grid;
  $("#sheet-name").textContent = asset.name;
  $("#sheet-dimensions").textContent = `${asset.width}×${asset.height} · ${asset.pack}`;
  const image = new Image();
  image.decoding = "async";
  image.addEventListener("load", () => { state.sheetImage = image; drawSheet(); });
  image.src = assetUrl(asset.path);
  renderAssets();
  updateSelectionDetails();
}

function sheetCell(event) {
  const canvas = $("#sheet-canvas");
  const rect = canvas.getBoundingClientRect();
  const px = (event.clientX - rect.left) * canvas.width / rect.width;
  const py = (event.clientY - rect.top) * canvas.height / rect.height;
  const grid = Number($("#source-grid").value) || 1;
  const columns = Math.max(1, Math.floor(canvas.width / grid));
  const rows = Math.max(1, Math.floor(canvas.height / grid));
  return {
    x: Math.max(0, Math.min(Math.floor(px / grid), columns - 1)),
    y: Math.max(0, Math.min(Math.floor(py / grid), rows - 1)),
  };
}

function drawSheet(preview = null) {
  if (!state.sheetImage) return;
  const canvas = $("#sheet-canvas");
  canvas.width = state.sheetImage.naturalWidth;
  canvas.height = state.sheetImage.naturalHeight;
  const context = canvas.getContext("2d");
  context.imageSmoothingEnabled = false;
  context.clearRect(0, 0, canvas.width, canvas.height);
  context.drawImage(state.sheetImage, 0, 0);
  const grid = Number($("#source-grid").value) || 1;
  context.strokeStyle = "rgba(235,216,172,.25)";
  context.lineWidth = 1;
  context.beginPath();
  for (let x = grid; x < canvas.width; x += grid) { context.moveTo(x + .5, 0); context.lineTo(x + .5, canvas.height); }
  for (let y = grid; y < canvas.height; y += grid) { context.moveTo(0, y + .5); context.lineTo(canvas.width, y + .5); }
  context.stroke();
  const selection = preview || state.selection;
  if (selection) {
    context.fillStyle = "rgba(224,184,92,.18)";
    context.strokeStyle = "#f1cf78";
    context.lineWidth = 3;
    context.fillRect(selection.x * grid, selection.y * grid, selection.width * grid, selection.height * grid);
    context.strokeRect(selection.x * grid + 1.5, selection.y * grid + 1.5, selection.width * grid - 3, selection.height * grid - 3);
  }
}

function rectangleFromCells(start, end) {
  const grid = Number($("#source-grid").value) || 1;
  return { x: Math.min(start.x, end.x), y: Math.min(start.y, end.y), width: Math.abs(end.x - start.x) + 1, height: Math.abs(end.y - start.y) + 1, grid };
}

function updateSelectionDetails() {
  if (!state.selection || !state.sheet) {
    $("#selection-title").textContent = "None";
    $("#selection-details").textContent = "Pick a sheet, then drag over the object or tile you want.";
    return;
  }
  const s = state.selection;
  const pixelWidth = s.width * s.grid;
  const pixelHeight = s.height * s.grid;
  $("#selection-title").textContent = `${pixelWidth}×${pixelHeight}px from ${state.sheet.name}`;
  $("#selection-details").textContent = `Source (${s.x}, ${s.y}) on a ${s.grid}px selection grid · rendered at its native pixel size on the ${state.layer} layer.`;
}

async function refreshLevels() {
  const response = await fetch("/api/levels");
  const data = await response.json();
  const select = $("#level-list");
  select.replaceChildren(...data.levels.map((id) => new Option(id, id)));
  if (data.levels.includes(state.room.id)) select.value = state.room.id;
}

async function saveRoom() {
  state.room.schema_version = 2;
  state.room.id = $("#room-id").value.trim();
  state.room.name = $("#room-name").value.trim();
  if (!/^[a-z0-9][a-z0-9_-]{0,63}$/.test(state.room.id)) { setStatus("Room ID must use lowercase letters, numbers, hyphens, or underscores."); return; }
  const response = await fetch(`/api/levels/${encodeURIComponent(state.room.id)}`, {
    method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify(state.room),
  });
  const result = await response.json();
  if (!response.ok) { setStatus(result.errors?.join(" · ") || "Save failed."); return; }
  setStatus(`Saved ${result.path}. Run “make assets” to rebuild the runtime room image.`);
  await refreshLevels();
}

async function loadRoom() {
  const id = $("#level-list").value;
  if (!id) return;
  const response = await fetch(`/api/levels/${encodeURIComponent(id)}`);
  if (!response.ok) { setStatus("Could not load that room."); return; }
  state.room = normalizeRoom(await response.json());
  state.undo = [];
  syncInputs(); drawRoom();
  setStatus(`Loaded ${id}.`);
}

function exportRoom() {
  const blob = new Blob([`${JSON.stringify(state.room, null, 2)}\n`], { type: "application/json" });
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob); link.download = `${state.room.id}.json`; link.click();
  URL.revokeObjectURL(link.href);
}

function bindEvents() {
  $("#asset-search").addEventListener("input", filterAssets);
  $("#pack-filter").addEventListener("change", filterAssets);
  $("#layer").addEventListener("change", (event) => { state.layer = event.target.value; updateSelectionDetails(); });
  $("#capture-damaged").addEventListener("click", () => captureStateSource("damaged"));
  $("#capture-repaired").addEventListener("click", () => captureStateSource("repaired"));
  $("#background").addEventListener("input", (event) => { state.room.background = event.target.value; drawRoom(); });
  $("#zoom").addEventListener("change", (event) => { state.zoom = Number(event.target.value); drawRoom(); });
  $("#source-grid").addEventListener("change", () => { state.selection = null; drawSheet(); updateSelectionDetails(); });
  $("#tool-group").addEventListener("click", (event) => {
    const button = event.target.closest("button[data-tool]"); if (!button) return;
    state.tool = button.dataset.tool;
    document.querySelectorAll("[data-tool]").forEach((item) => item.classList.toggle("active", item === button));
  });
  for (const selector of ["#room-width", "#room-height"]) $(selector).addEventListener("change", () => {
    pushUndo();
    state.room.grid.width = Math.max(4, Math.min(128, Number($("#room-width").value)));
    state.room.grid.height = Math.max(4, Math.min(128, Number($("#room-height").value)));
    drawRoom();
  });
  $("#room-id").addEventListener("change", (event) => { state.room.id = event.target.value.trim(); });
  $("#room-name").addEventListener("change", (event) => { state.room.name = event.target.value.trim(); });
  $("#new-room").addEventListener("click", () => {
    state.room = freshRoom();
    state.undo = [];
    state.stateSources = { damaged: null, repaired: null };
    syncInputs(); updateStateSourceDetails(); drawRoom(); setStatus("Started a new room.");
  });
  $("#save-room").addEventListener("click", saveRoom);
  $("#load-room").addEventListener("click", loadRoom);
  $("#export-room").addEventListener("click", exportRoom);
  $("#undo").addEventListener("click", undo);

  const roomCanvas = $("#room-canvas");
  roomCanvas.addEventListener("contextmenu", (event) => { event.preventDefault(); editRoom(roomCell(event), true); });
  roomCanvas.addEventListener("pointerdown", (event) => { if (event.button === 0) editRoom(roomCell(event)); });

  const sheetCanvas = $("#sheet-canvas");
  sheetCanvas.addEventListener("pointerdown", (event) => { if (!state.sheetImage) return; state.sheetDrag = sheetCell(event); sheetCanvas.setPointerCapture(event.pointerId); });
  sheetCanvas.addEventListener("pointermove", (event) => { if (state.sheetDrag) drawSheet(rectangleFromCells(state.sheetDrag, sheetCell(event))); });
  sheetCanvas.addEventListener("pointerup", (event) => {
    if (!state.sheetDrag) return;
    state.selection = rectangleFromCells(state.sheetDrag, sheetCell(event)); state.sheetDrag = null;
    drawSheet(); updateSelectionDetails();
  });
  window.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z") { event.preventDefault(); undo(); }
  });
}

async function initialize() {
  state.room = freshRoom(); syncInputs(); bindEvents(); drawRoom();
  const response = await fetch("/api/catalog");
  state.catalog = await response.json();
  const packFilter = $("#pack-filter");
  for (const pack of state.catalog.packs) packFilter.append(new Option(pack, pack));
  $("#asset-search").value = "motel";
  filterAssets();
  await refreshLevels();
  setStatus(`Ready · ${state.catalog.count.toLocaleString()} private images indexed.`);
}

initialize().catch((error) => { console.error(error); setStatus(`Editor failed to start: ${error.message}`); });
