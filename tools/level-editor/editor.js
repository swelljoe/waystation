const $ = (selector) => document.querySelector(selector);
const layerOrder = { floor: 0, wall: 1, object: 2, overlay: 3 };
const imageCache = new Map();

const state = {
  catalog: null,
  repairPairs: {},
  selectedRepairPair: null,
  filtered: [],
  sheet: null,
  sheetImage: null,
  sheetDrag: null,
  selection: null,
  tool: "stamp",
  layer: "floor",
  zoom: 1,
  snapGrid: 32,
  stampTransform: { flip_x: false, flip_y: false },
  collisionVisible: true,
  room: null,
  roomDrag: null,
  undo: [],
  stateSources: { damaged: null, repaired: null },
};

function freshRoom() {
  return {
    schema_version: 4,
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
  room.schema_version = 4;
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

function collisionCellsForRendering(room, visible) {
  return visible ? room.collision : [];
}

function authoredStampTransform() {
  return state.stampTransform.flip_x || state.stampTransform.flip_y
    ? structuredClone(state.stampTransform)
    : null;
}

function drawTransformedImage(context, image, sourceBox, destinationBox, transform = {}) {
  const flipX = transform.flip_x === true;
  const flipY = transform.flip_y === true;
  context.save();
  context.translate(
    destinationBox.x + (flipX ? destinationBox.width : 0),
    destinationBox.y + (flipY ? destinationBox.height : 0),
  );
  context.scale(flipX ? -1 : 1, flipY ? -1 : 1);
  context.drawImage(
    image,
    sourceBox.x,
    sourceBox.y,
    sourceBox.width,
    sourceBox.height,
    0,
    0,
    destinationBox.width,
    destinationBox.height,
  );
  context.restore();
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

function placementPixelPosition(placement, tileSize = roomTileSize()) {
  if (placement.position) {
    return {
      x: placement.position.x * placement.position.grid,
      y: placement.position.y * placement.position.grid,
    };
  }
  return {
    x: placement.x * tileSize,
    y: placement.y * tileSize,
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
      const template = templateForRoom(element.template);
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

function templateForRoom(templateId) {
  return state.repairPairs[templateId] || state.room.templates[templateId];
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

function roomSnapCell(event, snapGrid) {
  const canvas = $("#room-canvas");
  const rect = canvas.getBoundingClientRect();
  const pixelX = (event.clientX - rect.left) * canvas.width / rect.width;
  const pixelY = (event.clientY - rect.top) * canvas.height / rect.height;
  if (pixelX < 0 || pixelY < 0 || pixelX >= canvas.width || pixelY >= canvas.height) return null;
  const snap = snapGrid * state.zoom;
  return { x: Math.floor(pixelX / snap), y: Math.floor(pixelY / snap) };
}

function drawRoomGrid(context, canvas, spacing, color) {
  if (spacing <= 0) return;
  context.lineWidth = 1;
  context.strokeStyle = color;
  context.beginPath();
  for (let x = 0; x <= canvas.width; x += spacing) {
    context.moveTo(Math.round(x) + .5, 0);
    context.lineTo(Math.round(x) + .5, canvas.height);
  }
  for (let y = 0; y <= canvas.height; y += spacing) {
    context.moveTo(0, Math.round(y) + .5);
    context.lineTo(canvas.width, Math.round(y) + .5);
  }
  context.stroke();
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
    const position = placementPixelPosition(placement);
    const destinationX = position.x * state.zoom;
    const destinationY = position.y * state.zoom;
    if (placement.repeat) {
      const repeatWidth = placement.width * tile;
      const repeatHeight = placement.height * tile;
      context.save();
      context.beginPath();
      context.rect(destinationX, destinationY, repeatWidth, repeatHeight);
      context.clip();
      for (let y = 0; y < repeatHeight; y += sourceHeight * state.zoom) {
        for (let x = 0; x < repeatWidth; x += sourceWidth * state.zoom) {
          drawTransformedImage(
            context,
            image,
            { x: source.x * source.grid, y: source.y * source.grid, width: sourceWidth, height: sourceHeight },
            { x: destinationX + x, y: destinationY + y, width: sourceWidth * state.zoom, height: sourceHeight * state.zoom },
            placement.transform,
          );
        }
      }
      context.restore();
    } else {
      drawTransformedImage(
        context,
        image,
        { x: source.x * source.grid, y: source.y * source.grid, width: sourceWidth, height: sourceHeight },
        { x: destinationX, y: destinationY, width: sourceWidth * state.zoom, height: sourceHeight * state.zoom },
        placement.transform,
      );
    }
  }

  const snapSpacing = state.snapGrid * state.zoom;
  drawRoomGrid(context, canvas, snapSpacing, "rgba(224,196,134,.16)");
  if (state.snapGrid !== roomTileSize()) {
    drawRoomGrid(context, canvas, tile, "rgba(224,196,134,.3)");
  }

  const visibleCollision = collisionCellsForRendering(state.room, state.collisionVisible);
  if (visibleCollision.length) {
    context.fillStyle = "rgba(180,65,65,.42)";
    for (const cell of visibleCollision) context.fillRect(cell.x * tile, cell.y * tile, tile, tile);
  }
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
    const position = placementPixelPosition(p);
    const placementLeft = position.x;
    const placementTop = position.y;
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

function footprintForPixelSizes(pixelSizes, tileSize) {
  if (!pixelSizes.length) return null;
  return {
    width: Math.ceil(Math.max(...pixelSizes.map((size) => size.width)) / tileSize),
    height: Math.ceil(Math.max(...pixelSizes.map((size) => size.height)) / tileSize),
  };
}

function activeStampSpec() {
  const behavior = $("#behavior").value;
  const tileSize = roomTileSize();
  const snapGrid = state.snapGrid;
  if (behavior === "baked") {
    if (!state.selection) {
      setStatus("Select a source-sheet rectangle first.");
      return null;
    }
    const pixelSizes = [
      { width: state.selection.width * state.selection.grid, height: state.selection.height * state.selection.grid },
    ];
    const footprint = footprintForPixelSizes(pixelSizes, tileSize);
    const strokeFootprint = footprintForPixelSizes(pixelSizes, snapGrid);
    return { behavior, snapGrid, strokeFootprint, ...footprint };
  }
  const templateId = state.selectedRepairPair;
  const template = templateId ? templateForRoom(templateId) : null;
  if (!template) {
    setStatus("Select a saved repair pair from the library first.");
    return null;
  }
  const stateSizes = Object.values(template.states)
    .filter((visual) => visual.visible !== false && visual.source)
    .map((visual) => ({
      width: visual.source.width * visual.source.grid,
      height: visual.source.height * visual.source.grid,
    }));
  const footprint = footprintForPixelSizes(stateSizes, tileSize);
  if (!footprint) {
    setStatus(`Template ${templateId} has no visible state crops.`);
    return null;
  }
  return {
    behavior,
    snapGrid,
    strokeFootprint: footprintForPixelSizes(stateSizes, snapGrid),
    collection: behavior === "structure" ? "structures" : "fixtures",
    templateId,
    template,
    ...footprint,
  };
}

function editRoom(cell, forceErase = false, options = {}) {
  if (!cell) return false;
  const { recordUndo = true, collisionMode = null, stampSpec = null } = options;
  const tool = forceErase ? "erase" : state.tool;
  if (tool === "stamp") {
    const spec = stampSpec || activeStampSpec();
    if (!spec) return false;
    if (recordUndo) pushUndo();
    if (spec.behavior === "baked") {
      const s = state.selection;
      const transform = authoredStampTransform();
      const placement = {
        layer: state.layer,
        position: { grid: spec.snapGrid, x: cell.x, y: cell.y },
        width: spec.width,
        height: spec.height,
        source: selectionSource(),
      };
      if (transform) placement.transform = transform;
      state.room.placements.push(placement);
      setStatus(`Placed ${s.width * s.grid}×${s.height * s.grid}px baked stamp on a ${spec.snapGrid}px grid.`);
    } else {
      const id = nextElementId(spec.templateId);
      const transform = authoredStampTransform();
      const element = {
        id,
        template: spec.templateId,
        position: { grid: spec.snapGrid, x: cell.x, y: cell.y },
        width: spec.width,
        height: spec.height,
        initial_state: "damaged",
      };
      if (transform) element.transform = transform;
      state.room[spec.collection].push(element);
      setStatus(`Placed ${spec.template.label} as ${id} on a ${spec.snapGrid}px grid.`);
    }
  } else if (tool === "erase") {
    if (recordUndo) pushUndo();
    if (!eraseAt(cell)) {
      if (recordUndo) state.undo.pop();
      return false;
    }
  } else if (tool === "collision") {
    const index = state.room.collision.findIndex((item) => item.x === cell.x && item.y === cell.y);
    const mode = collisionMode || (index >= 0 ? "remove" : "add");
    if ((mode === "add" && index >= 0) || (mode === "remove" && index < 0)) return false;
    if (recordUndo) pushUndo();
    if (mode === "remove") state.room.collision.splice(index, 1);
    else state.room.collision.push(cell);
  } else if (tool === "entry") {
    if (recordUndo) pushUndo();
    state.room.entry = cell;
  } else if (tool === "exit") {
    const index = state.room.exits.findIndex((item) => item.x === cell.x && item.y === cell.y);
    if (recordUndo) pushUndo();
    if (index >= 0) state.room.exits.splice(index, 1);
    else state.room.exits.push({ ...cell, to: "exterior", spawn: "motel-door" });
  }
  drawRoom();
  return true;
}

function gridLine(start, end) {
  const cells = [];
  let x = start.x;
  let y = start.y;
  const dx = Math.abs(end.x - start.x);
  const sx = start.x < end.x ? 1 : -1;
  const dy = -Math.abs(end.y - start.y);
  const sy = start.y < end.y ? 1 : -1;
  let error = dx + dy;
  while (true) {
    cells.push({ x, y });
    if (x === end.x && y === end.y) break;
    const twiceError = 2 * error;
    if (twiceError >= dy) {
      error += dy;
      x += sx;
    }
    if (twiceError <= dx) {
      error += dx;
      y += sy;
    }
  }
  return cells;
}

function stampUnitForCell(origin, cell, footprint) {
  return {
    x: Math.floor((cell.x - origin.x) / footprint.width),
    y: Math.floor((cell.y - origin.y) / footprint.height),
  };
}

function stampAnchorForUnit(origin, unit, footprint) {
  return {
    x: origin.x + unit.x * footprint.width,
    y: origin.y + unit.y * footprint.height,
  };
}

function beginRoomStroke(event) {
  if (event.button !== 0) return;
  const tool = state.tool;
  const stampSpec = tool === "stamp" ? activeStampSpec() : null;
  if (tool === "stamp" && !stampSpec) return;
  const cell = tool === "stamp"
    ? roomSnapCell(event, stampSpec.snapGrid)
    : roomCell(event);
  if (!cell) return;
  const collisionIndex = tool === "collision"
    ? state.room.collision.findIndex((item) => item.x === cell.x && item.y === cell.y)
    : -1;
  const collisionMode = collisionIndex >= 0 ? "remove" : "add";
  pushUndo();
  const changed = editRoom(cell, false, { recordUndo: false, collisionMode, stampSpec });
  if (!changed) {
    state.undo.pop();
    return;
  }
  if (!["stamp", "erase", "collision"].includes(tool)) return;
  const canvas = $("#room-canvas");
  canvas.setPointerCapture(event.pointerId);
  if (tool === "stamp") {
    state.roomDrag = {
      pointerId: event.pointerId,
      kind: "stamp",
      origin: cell,
      footprint: stampSpec.strokeFootprint,
      stampSpec,
      lastUnit: { x: 0, y: 0 },
      visited: new Set(["0,0"]),
      count: 1,
    };
  } else {
    state.roomDrag = {
      pointerId: event.pointerId,
      kind: tool,
      collisionMode,
      lastCell: cell,
      visited: new Set([`${cell.x},${cell.y}`]),
      count: 1,
    };
  }
}

function continueRoomStroke(event) {
  const drag = state.roomDrag;
  if (!drag || drag.pointerId !== event.pointerId) return;
  const cell = drag.kind === "stamp"
    ? roomSnapCell(event, drag.stampSpec.snapGrid)
    : roomCell(event);
  if (!cell) return;
  if (drag.kind === "stamp") {
    const unit = stampUnitForCell(drag.origin, cell, drag.footprint);
    for (const nextUnit of gridLine(drag.lastUnit, unit).slice(1)) {
      const key = `${nextUnit.x},${nextUnit.y}`;
      if (drag.visited.has(key)) continue;
      drag.visited.add(key);
      const anchor = stampAnchorForUnit(drag.origin, nextUnit, drag.footprint);
      if (editRoom(anchor, false, { recordUndo: false, stampSpec: drag.stampSpec })) {
        drag.count += 1;
      }
    }
    drag.lastUnit = unit;
    return;
  }
  for (const nextCell of gridLine(drag.lastCell, cell).slice(1)) {
    const key = `${nextCell.x},${nextCell.y}`;
    if (drag.visited.has(key)) continue;
    drag.visited.add(key);
    const changed = editRoom(nextCell, false, {
      recordUndo: false,
      collisionMode: drag.collisionMode,
    });
    if (changed) drag.count += 1;
  }
  drag.lastCell = cell;
}

function finishRoomStroke(event) {
  const drag = state.roomDrag;
  if (!drag || drag.pointerId !== event.pointerId) return;
  state.roomDrag = null;
  const canvas = $("#room-canvas");
  if (canvas.hasPointerCapture(event.pointerId)) canvas.releasePointerCapture(event.pointerId);
  const label = drag.kind === "stamp" ? "stamps" : drag.kind === "collision" ? "collision cells" : "items";
  setStatus(`Painted ${drag.count} ${label} in one undoable stroke.`);
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

function repairPairMatches(pairId, pair, query) {
  const words = query.toLowerCase().trim().split(/\s+/).filter(Boolean);
  const sourcePaths = Object.values(pair.states || {})
    .map((visual) => visual.source?.path || "")
    .join(" ");
  const haystack = `${pairId} ${pair.label} ${pair.kind} ${pair.layer} ${sourcePaths}`.toLowerCase();
  return words.every((word) => haystack.includes(word));
}

function drawPairPreview(canvas, source) {
  const context = canvas.getContext("2d");
  context.imageSmoothingEnabled = false;
  context.fillStyle = "#090807";
  context.fillRect(0, 0, canvas.width, canvas.height);
  if (!source) return;
  const image = getImage(source.path);
  const paint = () => paintPairPreview(canvas, image, source, state.stampTransform);
  if (image.complete) paint();
  else image.addEventListener("load", paint, { once: true });
}

function paintPairPreview(canvas, image, source, transform = {}) {
  if (!image.naturalWidth) return false;
  const context = canvas.getContext("2d");
  context.imageSmoothingEnabled = false;
  const width = source.width * source.grid;
  const height = source.height * source.grid;
  const scale = Math.min(canvas.width / width, canvas.height / height);
  const drawWidth = Math.max(1, Math.floor(width * scale));
  const drawHeight = Math.max(1, Math.floor(height * scale));
  drawTransformedImage(
    context,
    image,
    { x: source.x * source.grid, y: source.y * source.grid, width, height },
    {
      x: Math.floor((canvas.width - drawWidth) / 2),
      y: Math.floor((canvas.height - drawHeight) / 2),
      width: drawWidth,
      height: drawHeight,
    },
    transform,
  );
  return true;
}

function renderRepairPairs() {
  const list = $("#pair-list");
  if (!list) return;
  const query = $("#pair-search").value;
  const pairs = Object.entries(state.repairPairs)
    .filter(([pairId, pair]) => repairPairMatches(pairId, pair, query))
    .sort((left, right) => left[1].label.localeCompare(right[1].label));
  list.replaceChildren();
  for (const [pairId, pair] of pairs) {
    const card = document.createElement("button");
    card.type = "button";
    card.className = `pair-card${state.selectedRepairPair === pairId ? " active" : ""}`;
    const previews = document.createElement("span");
    previews.className = "pair-previews";
    for (const stateName of ["damaged", "repaired"]) {
      const canvas = document.createElement("canvas");
      canvas.className = "pair-preview";
      canvas.width = 38;
      canvas.height = 38;
      canvas.title = stateName;
      previews.append(canvas);
      drawPairPreview(canvas, pair.states[stateName]?.source);
    }
    const meta = document.createElement("span");
    meta.className = "pair-meta";
    const name = document.createElement("strong");
    name.textContent = pair.label;
    const details = document.createElement("small");
    details.textContent = `${pairId} · ${pair.kind} · ${pair.layer}`;
    meta.append(name, details);
    card.append(previews, meta);
    card.addEventListener("click", () => selectRepairPair(pairId));
    list.append(card);
  }
  $("#pair-count").textContent = `${pairs.length} of ${Object.keys(state.repairPairs).length}`;
}

function selectRepairPair(pairId) {
  const pair = state.repairPairs[pairId];
  if (!pair) return;
  state.selectedRepairPair = pairId;
  state.stateSources = {
    damaged: structuredClone(pair.states.damaged?.source || null),
    repaired: structuredClone(pair.states.repaired?.source || null),
  };
  $("#pair-id").value = pairId;
  $("#pair-id").readOnly = true;
  $("#pair-label").value = pair.label;
  $("#pair-kind").value = pair.kind;
  $("#pair-layer").value = pair.layer;
  $("#pair-mode-title").textContent = pair.label;
  $("#duplicate-pair").disabled = false;
  $("#delete-pair").disabled = false;
  updateStateSourceDetails();
  renderRepairPairs();
  drawRoom();
  setStatus(`Selected repair pair ${pairId}; choose a repairable behavior and stamp it.`);
}

function newRepairPair() {
  state.selectedRepairPair = null;
  state.stateSources = { damaged: null, repaired: null };
  $("#pair-id").value = "";
  $("#pair-id").readOnly = false;
  $("#pair-label").value = "";
  $("#pair-kind").value = "";
  $("#pair-layer").value = state.layer;
  $("#pair-mode-title").textContent = "New pair";
  $("#duplicate-pair").disabled = true;
  $("#delete-pair").disabled = true;
  updateStateSourceDetails();
  renderRepairPairs();
  $("#pair-id").focus();
  setStatus("Creating a new repair pair; capture damaged and repaired crops, then save.");
}

function duplicateRepairPair() {
  if (!state.selectedRepairPair) return;
  const sourceId = state.selectedRepairPair;
  state.selectedRepairPair = null;
  $("#pair-id").value = "";
  $("#pair-id").readOnly = false;
  $("#pair-mode-title").textContent = `Copy of ${sourceId}`;
  $("#duplicate-pair").disabled = true;
  $("#delete-pair").disabled = true;
  renderRepairPairs();
  $("#pair-id").focus();
  setStatus(`Duplicated ${sourceId}; give this independent pair a new ID and replace either crop as needed.`);
}

async function saveRepairPair() {
  const pairId = $("#pair-id").value.trim();
  const label = $("#pair-label").value.trim();
  const kind = slugify($("#pair-kind").value);
  if (!/^[a-z0-9][a-z0-9_-]{0,63}$/.test(pairId)) {
    setStatus("Pair ID must use lowercase letters, numbers, hyphens, or underscores.");
    return;
  }
  if (!state.selectedRepairPair && state.repairPairs[pairId]) {
    setStatus(`Pair ${pairId} already exists; select it before editing.`);
    return;
  }
  if (!label || !kind) {
    setStatus("Repair pairs need both a label and a kind.");
    return;
  }
  if (!state.stateSources.damaged || !state.stateSources.repaired) {
    setStatus("Capture both damaged and repaired source rectangles first.");
    return;
  }
  const pair = {
    label,
    kind,
    layer: $("#pair-layer").value,
    states: {
      damaged: { source: structuredClone(state.stateSources.damaged) },
      repaired: { source: structuredClone(state.stateSources.repaired) },
    },
  };
  const response = await fetch(`/api/repair-pairs/${encodeURIComponent(pairId)}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(pair),
  });
  const result = await response.json();
  if (!response.ok) {
    setStatus(result.errors?.join(" · ") || "Repair pair save failed.");
    return;
  }
  state.repairPairs[pairId] = pair;
  state.selectedRepairPair = pairId;
  $("#pair-id").readOnly = true;
  $("#duplicate-pair").disabled = false;
  $("#delete-pair").disabled = false;
  $("#pair-mode-title").textContent = pair.label;
  renderRepairPairs();
  drawRoom();
  setStatus(`Saved repair pair ${pairId}; it is available to every room. Run “make assets” to rebuild runtime art.`);
}

async function deleteRepairPair() {
  const pairId = state.selectedRepairPair;
  if (!pairId) return;
  const currentInstances = [...state.room.structures, ...state.room.fixtures];
  if (currentInstances.some((element) => element.template === pairId)) {
    setStatus(`Cannot delete ${pairId}; the current room uses it.`);
    return;
  }
  if (!window.confirm(`Delete repair pair “${pairId}”?`)) return;
  const response = await fetch(`/api/repair-pairs/${encodeURIComponent(pairId)}`, { method: "DELETE" });
  const result = await response.json();
  if (!response.ok) {
    setStatus(result.errors?.join(" · ") || "Repair pair delete failed.");
    return;
  }
  delete state.repairPairs[pairId];
  newRepairPair();
  setStatus(`Deleted repair pair ${pairId}.`);
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
  const orientation = [
    state.stampTransform.flip_x ? "horizontal flip" : null,
    state.stampTransform.flip_y ? "vertical flip" : null,
  ].filter(Boolean).join(" + ") || "original orientation";
  $("#selection-details").textContent = `Source (${s.x}, ${s.y}) on a ${s.grid}px selection grid · ${state.snapGrid}px destination snap · native pixels on the ${state.layer} layer · ${orientation}.`;
}

async function refreshLevels() {
  const response = await fetch("/api/levels");
  const data = await response.json();
  const select = $("#level-list");
  select.replaceChildren(...data.levels.map((id) => new Option(id, id)));
  if (data.levels.includes(state.room.id)) select.value = state.room.id;
}

async function saveRoom() {
  state.room.schema_version = 4;
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

function toggleCollisionVisibility() {
  state.collisionVisible = !state.collisionVisible;
  const button = $("#toggle-collision");
  button.textContent = `Collision: ${state.collisionVisible ? "shown" : "hidden"}`;
  button.setAttribute("aria-pressed", String(state.collisionVisible));
  button.title = state.collisionVisible
    ? "Hide the collision overlay without changing collision data"
    : "Show the collision overlay";
  drawRoom();
  setStatus(`Collision overlay ${state.collisionVisible ? "shown" : "hidden"}; collision data is unchanged.`);
}

function toggleStampFlip(axis) {
  state.stampTransform[axis] = !state.stampTransform[axis];
  const controls = {
    flip_x: $("#flip-horizontal"),
    flip_y: $("#flip-vertical"),
  };
  for (const [key, button] of Object.entries(controls)) {
    button.classList.toggle("active", state.stampTransform[key]);
    button.setAttribute("aria-pressed", String(state.stampTransform[key]));
  }
  renderRepairPairs();
  updateSelectionDetails();
  const enabled = [
    state.stampTransform.flip_x ? "horizontal" : null,
    state.stampTransform.flip_y ? "vertical" : null,
  ].filter(Boolean).join(" and ") || "none";
  setStatus(`Stamp flips: ${enabled}. Repair-pair states share the same orientation.`);
}

function bindEvents() {
  $("#asset-search").addEventListener("input", filterAssets);
  $("#pack-filter").addEventListener("change", filterAssets);
  $("#layer").addEventListener("change", (event) => { state.layer = event.target.value; updateSelectionDetails(); });
  $("#capture-damaged").addEventListener("click", () => captureStateSource("damaged"));
  $("#capture-repaired").addEventListener("click", () => captureStateSource("repaired"));
  $("#pair-search").addEventListener("input", renderRepairPairs);
  $("#new-pair").addEventListener("click", newRepairPair);
  $("#duplicate-pair").addEventListener("click", duplicateRepairPair);
  $("#save-pair").addEventListener("click", saveRepairPair);
  $("#delete-pair").addEventListener("click", deleteRepairPair);
  $("#focus-repair-pairs").addEventListener("click", () => {
    $("#repair-pair-library").scrollIntoView({ behavior: "smooth", block: "start" });
    $("#pair-search").focus();
  });
  $("#background").addEventListener("input", (event) => { state.room.background = event.target.value; drawRoom(); });
  $("#zoom").addEventListener("change", (event) => { state.zoom = Number(event.target.value); drawRoom(); });
  $("#snap-grid").addEventListener("change", (event) => {
    state.snapGrid = Math.max(1, Math.min(256, Number(event.target.value) || roomTileSize()));
    event.target.value = state.snapGrid;
    drawRoom();
    updateSelectionDetails();
    setStatus(`Stamp snap grid set to ${state.snapGrid}px; collision remains on the ${roomTileSize()}px room grid.`);
  });
  $("#toggle-collision").addEventListener("click", toggleCollisionVisibility);
  $("#flip-horizontal").addEventListener("click", () => toggleStampFlip("flip_x"));
  $("#flip-vertical").addEventListener("click", () => toggleStampFlip("flip_y"));
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
    syncInputs(); drawRoom(); setStatus("Started a new room.");
  });
  $("#save-room").addEventListener("click", saveRoom);
  $("#load-room").addEventListener("click", loadRoom);
  $("#export-room").addEventListener("click", exportRoom);
  $("#undo").addEventListener("click", undo);

  const roomCanvas = $("#room-canvas");
  roomCanvas.addEventListener("contextmenu", (event) => { event.preventDefault(); editRoom(roomCell(event), true); });
  roomCanvas.addEventListener("pointerdown", beginRoomStroke);
  roomCanvas.addEventListener("pointermove", continueRoomStroke);
  roomCanvas.addEventListener("pointerup", finishRoomStroke);
  roomCanvas.addEventListener("pointercancel", finishRoomStroke);

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
  const [catalogResponse, pairResponse] = await Promise.all([
    fetch("/api/catalog"),
    fetch("/api/repair-pairs"),
  ]);
  state.catalog = await catalogResponse.json();
  const pairDocument = await pairResponse.json();
  state.repairPairs = pairDocument.pairs || {};
  const packFilter = $("#pack-filter");
  for (const pack of state.catalog.packs) packFilter.append(new Option(pack, pack));
  $("#asset-search").value = "motel";
  filterAssets();
  renderRepairPairs();
  const firstPair = Object.keys(state.repairPairs).sort()[0];
  if (firstPair) selectRepairPair(firstPair);
  else newRepairPair();
  await refreshLevels();
  setStatus(`Ready · ${state.catalog.count.toLocaleString()} private images indexed.`);
}

if (typeof window !== "undefined") {
  initialize().catch((error) => { console.error(error); setStatus(`Editor failed to start: ${error.message}`); });
}

if (typeof module !== "undefined") {
  module.exports = {
    collisionCellsForRendering,
    drawTransformedImage,
    footprintForPixelSizes,
    gridLine,
    paintPairPreview,
    placementPixelPosition,
    repairPairMatches,
    stampAnchorForUnit,
    stampUnitForCell,
  };
}
