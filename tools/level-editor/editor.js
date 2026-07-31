const $ = (selector) => document.querySelector(selector);
const layerOrder = { floor: 0, wall: 1, object: 2, overlay: 3 };
const imageCache = new Map();
const keyedImageCache = new Map();
const assetRevisions = new Map();

const state = {
  catalog: null,
  sceneType: "interior",
  repairPairs: {},
  selectedRepairPair: null,
  filtered: [],
  sheet: null,
  sheetImage: null,
  sheetDrag: null,
  smartSlice: null,
  selection: null,
  selectedPlaced: null,
  selectedRepairPreview: "scene",
  sceneRepairPreview: "authored",
  tool: "stamp",
  layer: "floor",
  zoom: 1,
  snapGrid: 32,
  collisionPen: 8,
  stampTransform: { flip_x: false, flip_y: false },
  gridVisible: true,
  collisionVisible: true,
  room: null,
  roomDrag: null,
  roomHoverPixel: null,
  undo: [],
  stateSources: { damaged: null, repaired: null },
};

function freshRoom(sceneType = "interior") {
  const shared = {
    schema_version: 5,
    collision: [],
    placements: [],
    items: [],
    templates: {},
    structures: [],
    fixtures: [],
  };
  if (sceneType === "building") {
    return {
      ...shared,
      scene_type: "building",
      id: "motel-exterior",
      name: "Abandoned Motel Exterior",
      grid: { width: 32, height: 20, tile_size: 32 },
    };
  }
  return {
    ...shared,
    id: "motel-room-01",
    name: "Motel Room 01",
    background: "#49382b",
    floor_line: "#34261d",
    grid: { width: 18, height: 11, tile_size: 32 },
    entry: { x: 8, y: 9 },
    exits: [{ x: 8, y: 10, to: "exterior", spawn: "motel-door" }],
  };
}

function normalizeRoom(room, sceneType = "interior") {
  room.schema_version = 5;
  if (sceneType === "building") room.scene_type = "building";
  room.placements ||= [];
  room.items ||= [];
  room.templates ||= {};
  room.structures ||= [];
  room.fixtures ||= [];
  room.collision ||= [];
  return room;
}

function assetUrl(path) {
  const base = `/asset/${path.split("/").map(encodeURIComponent).join("/")}`;
  const revision = assetRevisions.get(path);
  return revision ? `${base}?revision=${revision}` : base;
}

function invalidateAssetImage(path) {
  assetRevisions.set(path, (assetRevisions.get(path) || 0) + 1);
  imageCache.delete(path);
  for (const cacheKey of keyedImageCache.keys()) {
    if (cacheKey.startsWith(`${path}:`)) keyedImageCache.delete(cacheKey);
  }
  return assetRevisions.get(path);
}

function invalidateCatalogImages(paths) {
  for (const path of paths) {
    assetRevisions.set(path, (assetRevisions.get(path) || 0) + 1);
  }
  imageCache.clear();
  keyedImageCache.clear();
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

function applyBackgroundKey(imageData, key) {
  const [keyRed, keyGreen, keyBlue] = key.color;
  const tolerance = key.tolerance;
  const softness = Math.max(1, key.softness);
  for (let offset = 0; offset < imageData.data.length; offset += 4) {
    const distance = Math.max(
      Math.abs(imageData.data[offset] - keyRed),
      Math.abs(imageData.data[offset + 1] - keyGreen),
      Math.abs(imageData.data[offset + 2] - keyBlue),
    );
    const coverage = Math.max(0, Math.min(1, (distance - tolerance) / softness));
    imageData.data[offset + 3] = Math.round(imageData.data[offset + 3] * coverage);
  }
  return imageData;
}

function keyedImageForSource(image, source) {
  if (!source.background_key || !image.complete || !image.naturalWidth) return image;
  const cacheKey = `${source.path}:${JSON.stringify(source.background_key)}`;
  if (keyedImageCache.has(cacheKey)) return keyedImageCache.get(cacheKey);
  const canvas = document.createElement("canvas");
  canvas.width = image.naturalWidth;
  canvas.height = image.naturalHeight;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  context.drawImage(image, 0, 0);
  const pixels = context.getImageData(0, 0, canvas.width, canvas.height);
  context.putImageData(applyBackgroundKey(pixels, source.background_key), 0, 0);
  keyedImageCache.set(cacheKey, canvas);
  return canvas;
}

function detectSmartRegions(imageData) {
  const { width, height, data } = imageData;
  const pixelCount = width * height;
  let hasTransparency = false;
  const bins = new Uint32Array(4096);
  for (let index = 0; index < pixelCount; index++) {
    const offset = index * 4;
    if (data[offset + 3] < 250) hasTransparency = true;
    const bin = (data[offset] >> 4) << 8 | (data[offset + 1] >> 4) << 4 | data[offset + 2] >> 4;
    bins[bin] += 1;
  }

  let backgroundKey = null;
  if (!hasTransparency) {
    let dominantBin = 0;
    for (let index = 1; index < bins.length; index++) {
      if (bins[index] > bins[dominantBin]) dominantBin = index;
    }
    if (bins[dominantBin] / pixelCount >= .35) {
      let red = 0;
      let green = 0;
      let blue = 0;
      let samples = 0;
      for (let index = 0; index < pixelCount; index++) {
        const offset = index * 4;
        const bin = (data[offset] >> 4) << 8 | (data[offset + 1] >> 4) << 4 | data[offset + 2] >> 4;
        if (bin !== dominantBin) continue;
        red += data[offset];
        green += data[offset + 1];
        blue += data[offset + 2];
        samples += 1;
      }
      backgroundKey = {
        color: [Math.round(red / samples), Math.round(green / samples), Math.round(blue / samples)],
        tolerance: 24,
        softness: 16,
      };
    }
  }

  const foreground = new Uint8Array(pixelCount);
  for (let index = 0; index < pixelCount; index++) {
    const offset = index * 4;
    if (hasTransparency) {
      foreground[index] = data[offset + 3] > 16 ? 1 : 0;
    } else if (backgroundKey) {
      const distance = Math.max(
        Math.abs(data[offset] - backgroundKey.color[0]),
        Math.abs(data[offset + 1] - backgroundKey.color[1]),
        Math.abs(data[offset + 2] - backgroundKey.color[2]),
      );
      foreground[index] = distance > backgroundKey.tolerance ? 1 : 0;
    } else {
      foreground[index] = 1;
    }
  }
  if (backgroundKey) {
    for (let inset = 0; inset < Math.min(4, Math.floor(Math.min(width, height) / 2)); inset++) {
      const boundary = [];
      for (let x = inset; x < width - inset; x++) {
        boundary.push(inset * width + x, (height - 1 - inset) * width + x);
      }
      for (let y = inset + 1; y < height - 1 - inset; y++) {
        boundary.push(y * width + inset, y * width + width - 1 - inset);
      }
      const foregroundShare = boundary.filter((index) => foreground[index] === 1).length / boundary.length;
      if (foregroundShare < .8) break;
      for (const index of boundary) foreground[index] = 0;
    }
  }

  const queue = new Int32Array(pixelCount);
  const regions = [];
  for (let start = 0; start < pixelCount; start++) {
    if (foreground[start] !== 1) continue;
    foreground[start] = 2;
    let queueStart = 0;
    let queueEnd = 1;
    queue[0] = start;
    let pixels = 0;
    let minX = width;
    let minY = height;
    let maxX = 0;
    let maxY = 0;
    while (queueStart < queueEnd) {
      const current = queue[queueStart++];
      const x = current % width;
      const y = Math.floor(current / width);
      pixels += 1;
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
      for (let neighborY = Math.max(0, y - 1); neighborY <= Math.min(height - 1, y + 1); neighborY++) {
        for (let neighborX = Math.max(0, x - 1); neighborX <= Math.min(width - 1, x + 1); neighborX++) {
          const neighbor = neighborY * width + neighborX;
          if (foreground[neighbor] !== 1) continue;
          foreground[neighbor] = 2;
          queue[queueEnd++] = neighbor;
        }
      }
    }
    const region = { x: minX, y: minY, width: maxX - minX + 1, height: maxY - minY + 1, pixels };
    const isBackgroundFrame = backgroundKey
      && region.width >= width * .95
      && region.height >= height * .95;
    if (pixels >= 8 && !isBackgroundFrame) regions.push(region);
  }
  regions.sort((left, right) => left.y - right.y || left.x - right.x);
  return { regions, backgroundKey };
}

function setStatus(message) { $("#status").textContent = message; }

function syncInputs() {
  $("#room-id").value = state.room.id;
  $("#room-name").value = state.room.name;
  $("#room-width").value = state.room.grid.width;
  $("#room-height").value = state.room.grid.height;
  if (state.room.background) $("#background").value = state.room.background;
  $("#scene-type").value = state.sceneType;
  document.body.dataset.sceneType = state.sceneType;
  $("#editor-title").textContent = state.sceneType === "building" ? "Building Editor" : "Interior Editor";
}

function sceneEndpoint() {
  return state.sceneType === "building" ? "/api/buildings" : "/api/levels";
}

function sceneListKey() {
  return state.sceneType === "building" ? "buildings" : "levels";
}

async function switchSceneType(sceneType) {
  if (sceneType === state.sceneType) return;
  if (
    (state.room.placements.length || state.room.items.length || state.room.structures.length || state.room.fixtures.length)
    && !window.confirm("Switch scene types and start a new blank scene? Save first if needed.")
  ) {
    $("#scene-type").value = state.sceneType;
    return;
  }
  state.sceneType = sceneType;
  state.room = freshRoom(sceneType);
  clearPlacedSelection();
  state.roomHoverPixel = null;
  state.undo = [];
  state.tool = "stamp";
  document.querySelectorAll("[data-tool]").forEach((item) => item.classList.toggle("active", item.dataset.tool === "stamp"));
  $("#room-canvas").dataset.tool = state.tool;
  updateOrientationControls();
  updateLayerControl();
  syncInputs();
  if (state.catalog) {
    if (sceneType === "building") {
      $("#pack-filter").value = state.catalog.packs.includes("components") ? "components" : "";
      $("#asset-search").value = "";
    } else {
      $("#pack-filter").value = "";
      $("#asset-search").value = "motel";
    }
    $("#behavior").value = "baked";
    filterAssets();
  }
  drawRoom();
  await refreshLevels();
  setStatus(sceneType === "building"
    ? "Started a transparent building exterior; component sources are filtered, but all packs remain available."
    : "Started a new interior.");
}

function pushUndo() {
  state.undo.push(JSON.stringify(state.room));
  if (state.undo.length > 80) state.undo.shift();
}

function clearPlacedSelection() {
  state.selectedPlaced = null;
  state.selectedRepairPreview = "scene";
  updatePlacedSelectionInspector();
  updateOrientationControls();
}

function undo() {
  const previous = state.undo.pop();
  if (!previous) return;
  state.room = JSON.parse(previous);
  clearPlacedSelection();
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

function collisionAreaBounds(area, tileSize) {
  const grid = Number.isInteger(area.grid) ? area.grid : tileSize;
  return { x: area.x * grid, y: area.y * grid, width: grid, height: grid };
}

function rectanglesOverlap(left, right) {
  return left.x < right.x + right.width
    && left.x + left.width > right.x
    && left.y < right.y + right.height
    && left.y + left.height > right.y;
}

function rectangleContains(outer, inner) {
  return inner.x >= outer.x
    && inner.y >= outer.y
    && inner.x + inner.width <= outer.x + outer.width
    && inner.y + inner.height <= outer.y + outer.height;
}

function collisionBrush(cell, grid) {
  return { grid, x: cell.x, y: cell.y };
}

function collisionIndicesOverlapping(room, brush, tileSize) {
  const brushBounds = collisionAreaBounds(brush, tileSize);
  return room.collision
    .map((area, index) => rectanglesOverlap(collisionAreaBounds(area, tileSize), brushBounds) ? index : -1)
    .filter((index) => index >= 0);
}

function subtractCollisionArea(area, brush, tileSize) {
  const areaBounds = collisionAreaBounds(area, tileSize);
  const brushBounds = collisionAreaBounds(brush, tileSize);
  if (!rectanglesOverlap(areaBounds, brushBounds)) return [area];
  const areaGrid = Number.isInteger(area.grid) ? area.grid : tileSize;
  const brushGrid = brush.grid;
  if (areaGrid % brushGrid !== 0) return [];
  const remainder = [];
  for (let y = areaBounds.y; y < areaBounds.y + areaBounds.height; y += brushGrid) {
    for (let x = areaBounds.x; x < areaBounds.x + areaBounds.width; x += brushGrid) {
      const square = { x, y, width: brushGrid, height: brushGrid };
      if (!rectanglesOverlap(square, brushBounds)) {
        remainder.push({ grid: brushGrid, x: x / brushGrid, y: y / brushGrid });
      }
    }
  }
  return remainder;
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

function stampPreviewPlacement({ behavior, selection, layer, template, transform, snapGrid, cell }) {
  if (!cell) return null;
  const damagedVisual = template?.states?.damaged;
  const source = ["baked", "portable"].includes(behavior)
    ? selection && {
      path: selection.path,
      grid: selection.grid,
      x: selection.x,
      y: selection.y,
      width: selection.width,
      height: selection.height,
      ...(selection.background_key ? { background_key: structuredClone(selection.background_key) } : {}),
    }
    : damagedVisual?.visible === false ? null : damagedVisual?.source;
  if (!source) return null;
  const placement = {
    layer: ["baked", "portable"].includes(behavior) ? layer : template.layer,
    position: { grid: snapGrid, x: cell.x, y: cell.y },
    source: structuredClone(source),
  };
  if (transform?.flip_x || transform?.flip_y) placement.transform = structuredClone(transform);
  return placement;
}

function placementPixelSize(placement, tileSize = roomTileSize()) {
  if (placement.repeat) {
    return {
      width: placement.width * tileSize,
      height: placement.height * tileSize,
    };
  }
  return {
    width: placement.source.width * placement.source.grid,
    height: placement.source.height * placement.source.grid,
  };
}

function samePlacedItem(left, right) {
  return Boolean(left && right && left.collection === right.collection && left.index === right.index);
}

function repairStateForElement(element, scenePreview, selectedPreview, isSelected) {
  if (isSelected && selectedPreview !== "scene") return selectedPreview;
  if (scenePreview !== "authored") return scenePreview;
  return element.initial_state;
}

function effectivePlacementLayer(element, template = null) {
  return element.layer || template?.layer;
}

function setPlacementLayer(element, template, layer) {
  if (template?.layer === layer) delete element.layer;
  else element.layer = layer;
  return element;
}

function setPlacementOcclusion(element, enabled) {
  if (enabled) element.occludes_player = true;
  else delete element.occludes_player;
  return element;
}

function selectedElement() {
  if (!state.selectedPlaced) return null;
  return state.room[state.selectedPlaced.collection]?.[state.selectedPlaced.index] || null;
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
  state.room.items.forEach((item, index) => {
    renderables.push({ placement: item, collection: "items", index });
  });
  for (const collection of ["structures", "fixtures"]) {
    state.room[collection].forEach((element, index) => {
      const template = templateForRoom(element.template);
      const locator = { collection, index };
      const visualState = repairStateForElement(
        element,
        state.sceneRepairPreview,
        state.selectedRepairPreview,
        samePlacedItem(locator, state.selectedPlaced),
      );
      const visual = template?.states[visualState];
      if (!visual || visual.visible === false || !visual.source) return;
      renderables.push({
        placement: { ...element, layer: effectivePlacementLayer(element, template), source: visual.source },
        collection,
        index,
      });
    });
  }
  return renderables.sort((a, b) => layerOrder[a.placement.layer] - layerOrder[b.placement.layer]);
}

function findPlacedItemAtPixel(pixel, renderables, tileSize = roomTileSize()) {
  if (!pixel) return null;
  for (let index = renderables.length - 1; index >= 0; index--) {
    const renderable = renderables[index];
    const position = placementPixelPosition(renderable.placement, tileSize);
    const size = placementPixelSize(renderable.placement, tileSize);
    if (
      pixel.x >= position.x
      && pixel.x < position.x + size.width
      && pixel.y >= position.y
      && pixel.y < position.y + size.height
    ) return { collection: renderable.collection, index: renderable.index };
  }
  return null;
}

function selectedRoomRenderable(renderables) {
  return renderables.find((renderable) => samePlacedItem(renderable, state.selectedPlaced)) || null;
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

function roomPixel(event) {
  const canvas = $("#room-canvas");
  const rect = canvas.getBoundingClientRect();
  const x = ((event.clientX - rect.left) * canvas.width / rect.width) / state.zoom;
  const y = ((event.clientY - rect.top) * canvas.height / rect.height) / state.zoom;
  if (
    x < 0
    || y < 0
    || x >= state.room.grid.width * roomTileSize()
    || y >= state.room.grid.height * roomTileSize()
  ) return null;
  return { x, y };
}

function snapCellForPixel(pixel, snapGrid) {
  if (!pixel) return null;
  return { x: Math.floor(pixel.x / snapGrid), y: Math.floor(pixel.y / snapGrid) };
}

function draggedPlacementPosition(pixel, grabOffset, grid) {
  return {
    grid,
    x: Math.round((pixel.x - grabOffset.x) / grid),
    y: Math.round((pixel.y - grabOffset.y) / grid),
  };
}

function roomSnapCell(event, snapGrid) {
  return snapCellForPixel(roomPixel(event), snapGrid);
}

function activeStampPreview() {
  if (state.tool !== "stamp" || state.roomDrag) return null;
  const behavior = $("#behavior").value;
  const template = ["baked", "portable"].includes(behavior)
    ? null
    : templateForRoom(state.selectedRepairPair);
  return stampPreviewPlacement({
    behavior,
    selection: ["baked", "portable"].includes(behavior) && state.selection && state.sheet ? selectionSource() : null,
    layer: state.layer,
    template,
    transform: state.stampTransform,
    snapGrid: state.snapGrid,
    cell: snapCellForPixel(state.roomHoverPixel, state.snapGrid),
  });
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

function roomGridLayersForRendering(visible, snapGrid, tileSize, zoom) {
  if (!visible) return [];
  const layers = [{ spacing: snapGrid * zoom, color: "rgba(224,196,134,.16)" }];
  if (snapGrid !== tileSize) {
    layers.push({ spacing: tileSize * zoom, color: "rgba(224,196,134,.3)" });
  }
  return layers;
}

function drawRoom() {
  const canvas = $("#room-canvas");
  const tile = roomTileSize() * state.zoom;
  canvas.width = Math.round(state.room.grid.width * tile);
  canvas.height = Math.round(state.room.grid.height * tile);
  const context = canvas.getContext("2d");
  context.imageSmoothingEnabled = false;
  context.clearRect(0, 0, canvas.width, canvas.height);
  if (state.sceneType === "interior") {
    context.fillStyle = state.room.background;
    context.fillRect(0, 0, canvas.width, canvas.height);
  }

  const previewPlacement = activeStampPreview();
  const renderables = roomRenderables();
  if (previewPlacement) {
    renderables.push({ placement: previewPlacement, preview: true });
    renderables.sort((left, right) => (
      layerOrder[left.placement.layer] - layerOrder[right.placement.layer]
    ));
  }
  for (const { placement, preview = false } of renderables) {
    const sourceImage = getImage(placement.source.path);
    if (!sourceImage.complete || !sourceImage.naturalWidth) continue;
    const source = placement.source;
    const image = keyedImageForSource(sourceImage, source);
    const sourceWidth = source.width * source.grid;
    const sourceHeight = source.height * source.grid;
    const position = placementPixelPosition(placement);
    const destinationX = position.x * state.zoom;
    const destinationY = position.y * state.zoom;
    context.save();
    if (preview) context.globalAlpha = .62;
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
    context.restore();
  }

  for (const gridLayer of roomGridLayersForRendering(
    state.gridVisible,
    state.snapGrid,
    roomTileSize(),
    state.zoom,
  )) {
    drawRoomGrid(context, canvas, gridLayer.spacing, gridLayer.color);
  }

  const visibleCollision = collisionCellsForRendering(state.room, state.collisionVisible);
  if (visibleCollision.length) {
    context.fillStyle = "rgba(180,65,65,.42)";
    for (const area of visibleCollision) {
      const bounds = collisionAreaBounds(area, roomTileSize());
      context.fillRect(
        bounds.x * state.zoom,
        bounds.y * state.zoom,
        bounds.width * state.zoom,
        bounds.height * state.zoom,
      );
    }
  }
  if (state.tool === "collision" && state.roomHoverPixel) {
    const cell = snapCellForPixel(state.roomHoverPixel, state.collisionPen);
    const brush = collisionBrush(cell, state.collisionPen);
    const bounds = collisionAreaBounds(brush, roomTileSize());
    const removing = collisionIndicesOverlapping(state.room, brush, roomTileSize()).length > 0;
    context.fillStyle = removing ? "rgba(255,110,100,.28)" : "rgba(116,217,194,.28)";
    context.strokeStyle = removing ? "#ff8c80" : "#74d9c2";
    context.lineWidth = 1;
    context.fillRect(
      bounds.x * state.zoom,
      bounds.y * state.zoom,
      bounds.width * state.zoom,
      bounds.height * state.zoom,
    );
    context.strokeRect(
      Math.round(bounds.x * state.zoom) + 0.5,
      Math.round(bounds.y * state.zoom) + 0.5,
      Math.max(0, Math.round(bounds.width * state.zoom) - 1),
      Math.max(0, Math.round(bounds.height * state.zoom) - 1),
    );
  }
  if (previewPlacement) {
    const position = placementPixelPosition(previewPlacement);
    const size = placementPixelSize(previewPlacement);
    const x = position.x * state.zoom;
    const y = position.y * state.zoom;
    const width = size.width * state.zoom;
    const height = size.height * state.zoom;
    context.save();
    context.strokeStyle = "rgba(241,207,120,.9)";
    context.lineWidth = 1;
    context.setLineDash([5, 4]);
    context.strokeRect(Math.round(x) + .5, Math.round(y) + .5, Math.round(width), Math.round(height));
    context.setLineDash([]);
    context.fillStyle = "#f1cf78";
    context.fillRect(Math.round(x) - 2, Math.round(y) - 2, 5, 5);
    context.restore();
  }
  const selectedRenderable = selectedRoomRenderable(renderables);
  if (selectedRenderable) {
    const position = placementPixelPosition(selectedRenderable.placement);
    const size = placementPixelSize(selectedRenderable.placement);
    const x = position.x * state.zoom;
    const y = position.y * state.zoom;
    const width = size.width * state.zoom;
    const height = size.height * state.zoom;
    context.save();
    context.strokeStyle = "#74d9c2";
    context.lineWidth = 2;
    context.strokeRect(Math.round(x) + 1, Math.round(y) + 1, Math.max(0, Math.round(width) - 2), Math.max(0, Math.round(height) - 2));
    context.fillStyle = "#74d9c2";
    for (const [handleX, handleY] of [[x, y], [x + width, y], [x, y + height], [x + width, y + height]]) {
      context.fillRect(Math.round(handleX) - 3, Math.round(handleY) - 3, 7, 7);
    }
    context.restore();
  }
  if (state.sceneType === "interior") {
    context.font = `${Math.max(12, tile * .48)}px system-ui`;
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.fillStyle = "#e5c66e";
    context.fillText("E", (state.room.entry.x + .5) * tile, (state.room.entry.y + .5) * tile);
    context.fillStyle = "#7ed0b0";
    for (const exit of state.room.exits) context.fillText("⇩", (exit.x + .5) * tile, (exit.y + .5) * tile);
  }
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
      clearPlacedSelection();
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
  if (["baked", "portable"].includes(behavior)) {
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
  const {
    recordUndo = true,
    collisionMode = null,
    collisionGrid = state.collisionPen,
    stampSpec = null,
  } = options;
  const tool = forceErase ? "erase" : state.tool;
  if (tool === "stamp") {
    const spec = stampSpec || activeStampSpec();
    if (!spec) return false;
    if (recordUndo) pushUndo();
    if (["baked", "portable"].includes(spec.behavior)) {
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
      if (spec.behavior === "portable") {
        const tool = $("#portable-tool").value;
        const label = $("#portable-label").value.trim() || tool.replaceAll("_", " ");
        state.room.items.push({
          ...placement,
          id: nextPortableItemId(tool),
          label,
          tool,
          condition: $("#portable-condition").value,
        });
        setStatus(`Placed portable ${label}; its condition and location will persist independently from scenery.`);
      } else {
        state.room.placements.push(placement);
        setStatus(`Placed ${s.width * s.grid}×${s.height * s.grid}px baked stamp on a ${spec.snapGrid}px grid.`);
      }
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
    const brush = collisionBrush(cell, collisionGrid);
    const brushBounds = collisionAreaBounds(brush, roomTileSize());
    const overlaps = collisionIndicesOverlapping(state.room, brush, roomTileSize());
    const mode = collisionMode || (overlaps.length ? "remove" : "add");
    const alreadyCovered = state.room.collision.some((area) => (
      rectangleContains(collisionAreaBounds(area, roomTileSize()), brushBounds)
    ));
    if ((mode === "add" && alreadyCovered) || (mode === "remove" && !overlaps.length)) return false;
    if (recordUndo) pushUndo();
    if (mode === "remove") {
      state.room.collision = state.room.collision.flatMap((area) => (
        subtractCollisionArea(area, brush, roomTileSize())
      ));
    } else state.room.collision.push(brush);
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

function sameCell(left, right) {
  return left === right || (
    left !== null
    && right !== null
    && left.x === right.x
    && left.y === right.y
  );
}

function updateRoomHover(event) {
  const hoverGrid = state.tool === "collision" ? state.collisionPen : state.snapGrid;
  const previous = snapCellForPixel(state.roomHoverPixel, hoverGrid);
  state.roomHoverPixel = roomPixel(event);
  const next = snapCellForPixel(state.roomHoverPixel, hoverGrid);
  if (!sameCell(previous, next) && ["stamp", "collision"].includes(state.tool) && !state.roomDrag) drawRoom();
}

function clearRoomHover() {
  if (!state.roomHoverPixel) return;
  state.roomHoverPixel = null;
  if (["stamp", "collision"].includes(state.tool) && !state.roomDrag) drawRoom();
}

function beginRoomStroke(event) {
  if (event.button !== 0) return;
  const tool = state.tool;
  if (tool === "select") {
    const pixel = roomPixel(event);
    const locator = findPlacedItemAtPixel(pixel, roomRenderables());
    const changedSelection = !samePlacedItem(locator, state.selectedPlaced);
    state.selectedPlaced = locator;
    if (changedSelection) state.selectedRepairPreview = "scene";
    updatePlacedSelectionInspector();
    updateOrientationControls();
    drawRoom();
    if (!locator) {
      setStatus("No placed item at that point; selection cleared.");
      return;
    }
    const element = selectedElement();
    const position = placementPixelPosition(element);
    const canvas = $("#room-canvas");
    canvas.setPointerCapture(event.pointerId);
    canvas.classList.add("dragging-placement");
    state.roomDrag = {
      pointerId: event.pointerId,
      kind: "move",
      locator,
      grid: element.position?.grid || roomTileSize(),
      grabOffset: { x: pixel.x - position.x, y: pixel.y - position.y },
      originalRoom: JSON.stringify(state.room),
      changed: false,
    };
    setStatus("Selected placement. Drag to move it, use arrow keys to nudge it, or use the orientation and repair-preview controls.");
    return;
  }
  const stampSpec = tool === "stamp" ? activeStampSpec() : null;
  if (tool === "stamp" && !stampSpec) return;
  const collisionGrid = state.collisionPen;
  const cell = tool === "stamp"
    ? roomSnapCell(event, stampSpec.snapGrid)
    : tool === "collision"
      ? roomSnapCell(event, collisionGrid)
      : roomCell(event);
  if (!cell) return;
  const brush = tool === "collision" ? collisionBrush(cell, collisionGrid) : null;
  const collisionMode = brush && collisionIndicesOverlapping(state.room, brush, roomTileSize()).length
    ? "remove"
    : "add";
  pushUndo();
  const changed = editRoom(cell, false, {
    recordUndo: false,
    collisionMode,
    collisionGrid,
    stampSpec,
  });
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
    drawRoom();
  } else {
    state.roomDrag = {
      pointerId: event.pointerId,
      kind: tool,
      collisionMode,
      collisionGrid,
      lastCell: cell,
      visited: new Set([`${cell.x},${cell.y}`]),
      count: 1,
    };
  }
}

function continueRoomStroke(event) {
  const drag = state.roomDrag;
  if (!drag || drag.pointerId !== event.pointerId) return;
  if (drag.kind === "move") {
    const pixel = roomPixel(event);
    const element = selectedElement();
    if (!pixel || !element || !samePlacedItem(drag.locator, state.selectedPlaced)) return;
    const position = draggedPlacementPosition(pixel, drag.grabOffset, drag.grid);
    if (
      element.position?.grid === position.grid
      && element.position.x === position.x
      && element.position.y === position.y
    ) return;
    if (!drag.changed) {
      state.undo.push(drag.originalRoom);
      if (state.undo.length > 80) state.undo.shift();
      drag.changed = true;
    }
    element.position = position;
    delete element.x;
    delete element.y;
    updatePlacedSelectionInspector();
    drawRoom();
    return;
  }
  const cell = drag.kind === "stamp"
    ? roomSnapCell(event, drag.stampSpec.snapGrid)
    : drag.kind === "collision"
      ? roomSnapCell(event, drag.collisionGrid)
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
      collisionGrid: drag.collisionGrid,
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
  canvas.classList.remove("dragging-placement");
  if (canvas.hasPointerCapture(event.pointerId)) canvas.releasePointerCapture(event.pointerId);
  if (drag.kind === "move") {
    drawRoom();
    setStatus(drag.changed ? "Moved the selected placement." : "Selected placement without moving it.");
    return;
  }
  const label = drag.kind === "stamp" ? "stamps" : drag.kind === "collision" ? "collision cells" : "items";
  drawRoom();
  setStatus(`Painted ${drag.count} ${label} in one undoable stroke.`);
}

function selectionSource() {
  const s = state.selection;
  const source = { path: state.sheet.path, grid: s.grid, x: s.x, y: s.y, width: s.width, height: s.height };
  if (s.background_key) source.background_key = structuredClone(s.background_key);
  return source;
}

function slugify(value) {
  return value.toLowerCase().trim().replace(/[^a-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 52);
}

function nextElementId(kind) {
  const used = new Set([...state.room.structures, ...state.room.fixtures, ...state.room.items].map((element) => element.id));
  for (let sequence = 1; sequence < 10000; sequence++) {
    const candidate = `${kind}-${String(sequence).padStart(2, "0")}`;
    if (!used.has(candidate)) return candidate;
  }
  throw new Error(`Could not allocate an ID for ${kind}`);
}

function nextPortableItemId(tool) {
  const used = new Set([...state.room.items, ...state.room.structures, ...state.room.fixtures].map((item) => item.id));
  for (let sequence = 1; sequence < 10000; sequence++) {
    const candidate = `${tool}-${String(sequence).padStart(2, "0")}`;
    if (!used.has(candidate)) return candidate;
  }
  throw new Error(`Could not allocate an ID for ${tool}`);
}

function captureStateSource(stateName) {
  if (!state.selection || !state.sheet) {
    setStatus("Select a source-sheet rectangle first.");
    return;
  }
  state.stateSources[stateName] = selectionSource();
  if (stateName === "repaired") $("#repaired-invisible").checked = false;
  updateStateSourceDetails();
  setStatus(`Captured ${stateName} crop at ${state.selection.width * state.selection.grid}×${state.selection.height * state.selection.grid}px.`);
}

function updateStateSourceDetails() {
  for (const stateName of ["damaged", "repaired"]) {
    const source = state.stateSources[stateName];
    const invisible = stateName === "repaired" && $("#repaired-invisible")?.checked;
    $(`#${stateName}-source`).textContent = invisible
      ? "invisible when completed"
      : source
      ? `${source.path.split("/").at(-1)} · ${source.width * source.grid}×${source.height * source.grid}px`
      : "not captured";
  }
}

function repairPairMatches(pairId, pair, query) {
  const words = query.toLowerCase().trim().split(/\s+/).filter(Boolean);
  const sourcePaths = Object.values(pair.states || {})
    .map((visual) => visual.source?.path || "")
    .join(" ");
  const taskText = JSON.stringify(pair.task || {});
  const haystack = `${pairId} ${pair.label} ${pair.kind} ${pair.layer} ${sourcePaths} ${taskText}`.toLowerCase();
  return words.every((word) => haystack.includes(word));
}

function defaultTaskForKind(kind) {
  if (kind === "debris") return { action: "clean", skill: "upkeep", level: 0, tools: [], supplies: [], xp: 1 };
  if (kind === "floor") return { action: "repair", skill: "carpentry", level: 0, tools: ["hammer"], supplies: [{ item: "plank", amount: 1 }], xp: 1 };
  if (["furniture", "furnitute", "bed", "nightstand"].includes(kind)) return { action: "repair", skill: "carpentry", level: 1, tools: ["hammer"], supplies: [{ item: "plank", amount: 1 }], xp: 1 };
  if (["wall", "fireplace"].includes(kind)) return { action: "repair", skill: "masonry", level: 0, tools: ["trowel"], supplies: [{ item: "stone", amount: 1 }], xp: 1 };
  if (kind === "chimney") return { action: "clear", skill: "upkeep", level: 1, tools: ["ladder"], supplies: [], xp: 1 };
  if (kind === "roof") return { action: "repair", skill: "roofing", level: 0, tools: ["hammer", "ladder"], supplies: [{ item: "plank", amount: 1 }], xp: 1 };
  if (["door", "window", "mirror", "lamp"].includes(kind)) return { action: "repair", skill: "upkeep", level: 1, tools: ["hammer"], supplies: [{ item: "nails", amount: 1 }], xp: 1 };
  return { action: "repair", skill: "upkeep", level: 1, tools: [], supplies: [], xp: 1 };
}

function populateTaskControls(task) {
  const resolved = task || defaultTaskForKind(slugify($("#pair-kind").value));
  $("#pair-action").value = resolved.action || "repair";
  $("#pair-skill").value = resolved.skill || "upkeep";
  $("#pair-level").value = resolved.level ?? 0;
  $("#pair-xp").value = resolved.xp ?? 1;
  for (const input of document.querySelectorAll("#pair-tools input")) {
    input.checked = (resolved.tools || []).includes(input.value);
  }
  const costs = new Map((resolved.supplies || []).map((cost) => [cost.item, cost.amount]));
  for (const input of document.querySelectorAll("#pair-supplies input")) {
    input.value = costs.get(input.dataset.supply) || 0;
  }
}

function taskFromControls() {
  const tools = [...document.querySelectorAll("#pair-tools input:checked")].map((input) => input.value);
  const supplies = [...document.querySelectorAll("#pair-supplies input")]
    .map((input) => ({ item: input.dataset.supply, amount: Number.parseInt(input.value, 10) || 0 }))
    .filter((cost) => cost.amount > 0);
  return {
    action: $("#pair-action").value,
    skill: $("#pair-skill").value,
    level: Number.parseInt($("#pair-level").value, 10) || 0,
    tools,
    supplies,
    xp: Number.parseInt($("#pair-xp").value, 10) || 0,
  };
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
  const renderableImage = keyedImageForSource(image, source);
  const width = source.width * source.grid;
  const height = source.height * source.grid;
  const scale = Math.min(canvas.width / width, canvas.height / height);
  const drawWidth = Math.max(1, Math.floor(width * scale));
  const drawHeight = Math.max(1, Math.floor(height * scale));
  drawTransformedImage(
    context,
    renderableImage,
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
    const task = pair.task || defaultTaskForKind(pair.kind);
    details.textContent = `${pairId} · ${task.action} · ${task.skill} ${task.level}`;
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
  $("#repaired-invisible").checked = pair.states.repaired?.visible === false;
  populateTaskControls(pair.task || defaultTaskForKind(pair.kind));
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
  $("#repaired-invisible").checked = false;
  populateTaskControls(defaultTaskForKind(""));
  $("#pair-mode-title").textContent = "New pair";
  $("#duplicate-pair").disabled = true;
  $("#delete-pair").disabled = true;
  updateStateSourceDetails();
  renderRepairPairs();
  drawRoom();
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
  drawRoom();
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
  if (!state.stateSources.damaged || (!state.stateSources.repaired && !$("#repaired-invisible").checked)) {
    setStatus("Capture a damaged source and either capture repaired art or mark the completed state invisible.");
    return;
  }
  const pair = {
    label,
    kind,
    layer: $("#pair-layer").value,
    task: taskFromControls(),
    states: {
      damaged: { source: structuredClone(state.stateSources.damaged) },
      repaired: $("#repaired-invisible").checked
        ? { visible: false }
        : { source: structuredClone(state.stateSources.repaired) },
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
  setStatus(`Saved repair pair ${pairId}; it is available to every room and building. Run “make assets” to rebuild runtime art.`);
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

function populatePackFilter(preferredPack = "") {
  const packFilter = $("#pack-filter");
  packFilter.replaceChildren(new Option("All packs", ""));
  for (const pack of state.catalog.packs) packFilter.append(new Option(pack, pack));
  packFilter.value = state.catalog.packs.includes(preferredPack) ? preferredPack : "";
}

async function refreshAssetCatalog() {
  const button = $("#refresh-catalog");
  if (!state.catalog) return;
  const previousCatalog = state.catalog;
  const previousPaths = new Set(previousCatalog.assets.map((asset) => asset.path));
  const preferredPack = $("#pack-filter").value;
  const selectedPath = state.sheet?.path || null;
  button.disabled = true;
  setStatus("Rescanning the private asset library…");
  try {
    const response = await fetch("/api/catalog/refresh", { method: "POST" });
    if (!response.ok) throw new Error(`catalog refresh returned ${response.status}`);
    const catalog = await response.json();
    const currentPaths = new Set(catalog.assets.map((asset) => asset.path));
    const added = [...currentPaths].filter((path) => !previousPaths.has(path)).length;
    const removed = [...previousPaths].filter((path) => !currentPaths.has(path)).length;
    invalidateCatalogImages(new Set([...previousPaths, ...currentPaths]));
    state.catalog = catalog;
    populatePackFilter(preferredPack);

    const selectedAsset = selectedPath
      ? catalog.assets.find((asset) => asset.path === selectedPath)
      : null;
    if (selectedAsset) {
      state.sheet = selectedAsset;
      $("#sheet-name").textContent = selectedAsset.name;
      $("#sheet-dimensions").textContent = `${selectedAsset.width}×${selectedAsset.height} · ${selectedAsset.pack}`;
      loadSelectedSheetImage(selectedAsset, true, catalogRefreshStatus(catalog.count, added, removed));
    } else if (selectedPath) {
      state.sheet = null;
      state.sheetImage = null;
      state.selection = null;
      state.smartSlice = null;
      $("#sheet-name").textContent = "Choose an asset";
      $("#sheet-dimensions").textContent = "";
      $("#smart-slice").disabled = true;
      $("#refresh-sheet").disabled = true;
      const canvas = $("#sheet-canvas");
      canvas.getContext("2d").clearRect(0, 0, canvas.width, canvas.height);
    }
    filterAssets();
    updateSelectionDetails();
    renderRepairPairs();
    drawRoom();
    if (!selectedAsset) setStatus(catalogRefreshStatus(catalog.count, added, removed));
  } catch (error) {
    console.error(error);
    setStatus(`Could not refresh the asset library: ${error.message}`);
  } finally {
    button.disabled = false;
  }
}

function catalogRefreshStatus(count, added, removed) {
  const changes = [`${added.toLocaleString()} added`, `${removed.toLocaleString()} removed`];
  return `Asset library refreshed · ${count.toLocaleString()} images indexed · ${changes.join(" · ")}.`;
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

function loadSelectedSheetImage(asset, refreshed = false, refreshedStatus = null) {
  state.sheetImage = null;
  $("#smart-slice").disabled = true;
  $("#refresh-sheet").disabled = true;
  const image = getImage(asset.path);
  const finish = () => {
    if (state.sheet?.id !== asset.id) return;
    state.sheetImage = image;
    asset.width = image.naturalWidth;
    asset.height = image.naturalHeight;
    const selection = state.selection;
    if (
      selection
      && (
        (selection.x + selection.width) * selection.grid > image.naturalWidth
        || (selection.y + selection.height) * selection.grid > image.naturalHeight
      )
    ) state.selection = null;
    $("#sheet-dimensions").textContent = `${asset.width}×${asset.height} · ${asset.pack}`;
    $("#smart-slice").disabled = false;
    $("#refresh-sheet").disabled = false;
    renderAssets();
    if (refreshed) renderRepairPairs();
    drawSheet();
    updateSelectionDetails();
    drawRoom();
    if (refreshed) {
      setStatus(refreshedStatus || `Reloaded ${asset.name} from disk; sheet, room, and repair-pair previews now use the new pixels.`);
    }
  };
  if (image.complete && image.naturalWidth) finish();
  else image.addEventListener("load", finish, { once: true });
  image.addEventListener("error", () => {
    if (state.sheet?.id !== asset.id) return;
    $("#refresh-sheet").disabled = false;
    setStatus(`Could not reload ${asset.name}.`);
  }, { once: true });
}

function selectSheet(asset) {
  state.sheet = asset;
  state.selection = null;
  state.smartSlice = null;
  $("#smart-slice").classList.remove("active");
  $("#source-grid").value = asset.grid;
  $("#sheet-name").textContent = asset.name;
  $("#sheet-dimensions").textContent = `${asset.width}×${asset.height} · ${asset.pack}`;
  renderAssets();
  updateSelectionDetails();
  drawRoom();
  loadSelectedSheetImage(asset);
}

function refreshSheetImage() {
  if (!state.sheet) {
    setStatus("Choose a source sheet first.");
    return;
  }
  const asset = state.sheet;
  invalidateAssetImage(asset.path);
  state.smartSlice = null;
  $("#smart-slice").classList.remove("active");
  const canvas = $("#sheet-canvas");
  canvas.getContext("2d").clearRect(0, 0, canvas.width, canvas.height);
  renderAssets();
  renderRepairPairs();
  drawRoom();
  setStatus(`Reloading ${asset.name} from disk…`);
  loadSelectedSheetImage(asset, true);
}

function sheetPixel(event) {
  const canvas = $("#sheet-canvas");
  const rect = canvas.getBoundingClientRect();
  return {
    x: Math.max(0, Math.min(canvas.width - 1, Math.floor((event.clientX - rect.left) * canvas.width / rect.width))),
    y: Math.max(0, Math.min(canvas.height - 1, Math.floor((event.clientY - rect.top) * canvas.height / rect.height))),
  };
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
  if (state.smartSlice) {
    context.strokeStyle = "rgba(103,207,184,.72)";
    context.lineWidth = 1;
    for (const region of state.smartSlice.regions) {
      context.strokeRect(region.x + .5, region.y + .5, region.width - 1, region.height - 1);
    }
  }
  const selection = preview || state.selection;
  if (selection) {
    const selectionGrid = selection.grid;
    context.fillStyle = "rgba(224,184,92,.18)";
    context.strokeStyle = "#f1cf78";
    context.lineWidth = 3;
    context.fillRect(selection.x * selectionGrid, selection.y * selectionGrid, selection.width * selectionGrid, selection.height * selectionGrid);
    context.strokeRect(selection.x * selectionGrid + 1.5, selection.y * selectionGrid + 1.5, selection.width * selectionGrid - 3, selection.height * selectionGrid - 3);
  }
}

function toggleSmartSlice() {
  if (!state.sheetImage) return;
  if (state.smartSlice) {
    state.smartSlice = null;
    $("#smart-slice").classList.remove("active");
    drawSheet();
    setStatus("Smart slice hidden; drag on the source grid to select manually.");
    return;
  }
  setStatus("Detecting separated source-sheet components…");
  window.requestAnimationFrame(() => {
    const canvas = document.createElement("canvas");
    canvas.width = state.sheetImage.naturalWidth;
    canvas.height = state.sheetImage.naturalHeight;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    context.drawImage(state.sheetImage, 0, 0);
    state.smartSlice = detectSmartRegions(context.getImageData(0, 0, canvas.width, canvas.height));
    $("#smart-slice").classList.add("active");
    drawSheet();
    const keyed = state.smartSlice.backgroundKey ? " and removed its solid background" : "";
    setStatus(`Found ${state.smartSlice.regions.length} separated components${keyed}; click a teal box to select it.`);
  });
}

function selectSmartRegion(event) {
  if (!state.smartSlice) return false;
  const point = sheetPixel(event);
  const candidates = state.smartSlice.regions.filter((region) => (
    point.x >= region.x
    && point.x < region.x + region.width
    && point.y >= region.y
    && point.y < region.y + region.height
  ));
  if (!candidates.length) return false;
  candidates.sort((left, right) => left.width * left.height - right.width * right.height);
  const region = candidates[0];
  state.selection = {
    x: region.x,
    y: region.y,
    width: region.width,
    height: region.height,
    grid: 1,
  };
  if (state.smartSlice.backgroundKey) {
    state.selection.background_key = structuredClone(state.smartSlice.backgroundKey);
  }
  drawSheet();
  updateSelectionDetails();
  drawRoom();
  setStatus(`Smart-selected ${region.width}×${region.height}px component at (${region.x}, ${region.y}).`);
  return true;
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
  const keyed = s.background_key ? " · solid background removed" : "";
  $("#selection-details").textContent = `Source (${s.x}, ${s.y}) on a ${s.grid}px selection grid · ${state.snapGrid}px destination snap · native pixels on the ${state.layer} layer · ${orientation}${keyed}.`;
}

async function refreshLevels() {
  const response = await fetch(sceneEndpoint());
  const data = await response.json();
  const select = $("#level-list");
  const ids = data[sceneListKey()];
  select.replaceChildren(...ids.map((id) => new Option(id, id)));
  if (ids.includes(state.room.id)) select.value = state.room.id;
}

async function saveRoom() {
  state.room.schema_version = 5;
  if (state.sceneType === "building") state.room.scene_type = "building";
  else delete state.room.scene_type;
  state.room.id = $("#room-id").value.trim();
  state.room.name = $("#room-name").value.trim();
  if (!/^[a-z0-9][a-z0-9_-]{0,63}$/.test(state.room.id)) { setStatus("Scene ID must use lowercase letters, numbers, hyphens, or underscores."); return; }
  const response = await fetch(`${sceneEndpoint()}/${encodeURIComponent(state.room.id)}`, {
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
  const response = await fetch(`${sceneEndpoint()}/${encodeURIComponent(id)}`);
  if (!response.ok) { setStatus("Could not load that scene."); return; }
  state.room = normalizeRoom(await response.json(), state.sceneType);
  clearPlacedSelection();
  state.roomHoverPixel = null;
  state.undo = [];
  syncInputs(); drawRoom();
  setStatus(`Loaded ${state.sceneType === "building" ? "building" : "interior"} ${id}.`);
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

function toggleGridVisibility() {
  state.gridVisible = !state.gridVisible;
  const button = $("#toggle-grid");
  button.textContent = `Grid: ${state.gridVisible ? "shown" : "hidden"}`;
  button.setAttribute("aria-pressed", String(state.gridVisible));
  button.title = state.gridVisible
    ? "Hide edit-window grid lines without changing snapping"
    : "Show edit-window grid lines";
  drawRoom();
  setStatus(`Edit-window grid ${state.gridVisible ? "shown" : "hidden"}; snapping and scene data are unchanged.`);
}

function updateOrientationControls() {
  const editingSelection = state.tool === "select";
  const element = editingSelection ? selectedElement() : null;
  const transform = element?.transform || (editingSelection ? {} : state.stampTransform);
  const controls = {
    flip_x: $("#flip-horizontal"),
    flip_y: $("#flip-vertical"),
  };
  for (const [axis, button] of Object.entries(controls)) {
    button.disabled = editingSelection && !element;
    button.classList.toggle("active", transform[axis] === true);
    button.setAttribute("aria-pressed", String(transform[axis] === true));
    button.title = editingSelection
      ? `Flip the selected placement ${axis === "flip_x" ? "horizontally" : "vertically"}`
      : `Flip the next stamp ${axis === "flip_x" ? "horizontally" : "vertically"}`;
  }
}

function updateLayerControl() {
  const control = $("#layer");
  const editingSelection = state.tool === "select";
  const element = editingSelection ? selectedElement() : null;
  control.disabled = editingSelection && !element;
  if (element) {
    const template = state.selectedPlaced.collection === "placements"
      ? null
      : templateForRoom(element.template);
    control.value = effectivePlacementLayer(element, template);
    control.title = "Change the selected placement's layer · bottom to top: Floor, Wall, Object, Overlay";
  } else {
    control.value = state.layer;
    control.title = "Layer for the next stamp · bottom to top: Floor, Wall, Object, Overlay";
  }
}

function toggleStampFlip(axis) {
  if (state.tool === "select") {
    const element = selectedElement();
    if (!element) {
      setStatus("Select a placed item before changing its orientation.");
      return;
    }
    pushUndo();
    const transform = { ...(element.transform || {}) };
    transform[axis] = transform[axis] !== true;
    if (transform.flip_x || transform.flip_y) element.transform = transform;
    else delete element.transform;
    updateOrientationControls();
    updatePlacedSelectionInspector();
    drawRoom();
    setStatus(`Updated the selected placement's ${axis === "flip_x" ? "horizontal" : "vertical"} orientation.`);
    return;
  }
  state.stampTransform[axis] = !state.stampTransform[axis];
  updateOrientationControls();
  renderRepairPairs();
  updateSelectionDetails();
  drawRoom();
  const enabled = [
    state.stampTransform.flip_x ? "horizontal" : null,
    state.stampTransform.flip_y ? "vertical" : null,
  ].filter(Boolean).join(" and ") || "none";
  setStatus(`Stamp flips: ${enabled}. Repair-pair states share the same orientation.`);
}

function updatePlacedSelectionInspector() {
  const element = selectedElement();
  const title = $("#placed-selection-title");
  const details = $("#placed-selection-details");
  const clearButton = $("#clear-placement-selection");
  const previewButtons = document.querySelectorAll("[data-placement-preview]");
  const occlusionControl = $("#placement-occludes-player");
  const portableButton = $("#toggle-portable");
  if (!element) {
    title.textContent = "None";
    details.textContent = "Choose Select, then click an item in the scene.";
    clearButton.disabled = true;
    occlusionControl.disabled = true;
    occlusionControl.checked = false;
    portableButton.disabled = true;
    portableButton.textContent = "Make selected portable";
    for (const button of previewButtons) {
      button.disabled = true;
      button.classList.toggle("active", button.dataset.placementPreview === "scene");
    }
    updateLayerControl();
    return;
  }

  const { collection } = state.selectedPlaced;
  const repairable = collection === "structures" || collection === "fixtures";
  const portable = collection === "items";
  const template = repairable ? templateForRoom(element.template) : null;
  const layer = effectivePlacementLayer(element, template);
  const position = placementPixelPosition(element);
  const grid = element.position?.grid || roomTileSize();
  const flips = [
    element.transform?.flip_x ? "flipped H" : null,
    element.transform?.flip_y ? "flipped V" : null,
  ].filter(Boolean).join(" + ") || "original orientation";
  title.textContent = repairable
    ? `${template?.label || element.template} · ${element.id}`
    : portable
      ? `${element.label} · ${element.id}`
      : `Baked scenery #${state.selectedPlaced.index + 1}`;
  const placementType = repairable ? "Repairable" : portable ? `Portable ${element.tool}` : "Baked";
  const condition = portable ? ` · ${element.condition}` : "";
  details.textContent = `${placementType} ${collection.slice(0, -1)} on the ${layer} layer at (${position.x}, ${position.y})px · ${grid}px movement grid · ${flips}${condition}. Drag it or use arrow keys to reposition it.`;
  clearButton.disabled = false;
  occlusionControl.disabled = portable;
  occlusionControl.checked = element.occludes_player === true;
  portableButton.disabled = repairable;
  portableButton.textContent = portable ? "Make selected baked scenery" : "Make selected portable";
  if (portable) {
    $("#portable-tool").value = element.tool;
    $("#portable-label").value = element.label;
    $("#portable-condition").value = element.condition;
  }
  for (const button of previewButtons) {
    button.disabled = !repairable;
    button.classList.toggle("active", button.dataset.placementPreview === state.selectedRepairPreview);
  }
  updateLayerControl();
}

function toggleSelectedPortable() {
  const element = selectedElement();
  if (!element || !state.selectedPlaced || ["structures", "fixtures"].includes(state.selectedPlaced.collection)) {
    setStatus("Select baked scenery or a portable tool first.");
    return;
  }
  pushUndo();
  const { collection, index } = state.selectedPlaced;
  state.room[collection].splice(index, 1);
  if (collection === "items") {
    const { id: _id, label: _label, tool: _tool, condition: _condition, ...placement } = element;
    state.room.placements.push(placement);
    state.selectedPlaced = { collection: "placements", index: state.room.placements.length - 1 };
    setStatus("Returned the selected tool to baked scenery; it will no longer have gameplay state.");
  } else {
    const tool = $("#portable-tool").value;
    const label = $("#portable-label").value.trim() || tool.replaceAll("_", " ");
    state.room.items.push({
      ...element,
      id: nextPortableItemId(tool),
      label,
      tool,
      condition: $("#portable-condition").value,
    });
    state.selectedPlaced = { collection: "items", index: state.room.items.length - 1 };
    setStatus(`Promoted ${label} to a portable tool with persistent condition and location.`);
  }
  updatePlacedSelectionInspector();
  updateOrientationControls();
  updateLayerControl();
  drawRoom();
}

function updateSelectedPortableField(field, value) {
  const element = selectedElement();
  if (!element || state.selectedPlaced?.collection !== "items" || element[field] === value) return;
  pushUndo();
  element[field] = value;
  updatePlacedSelectionInspector();
  drawRoom();
  setStatus(`Updated the portable tool's ${field}.`);
}

function nudgeSelectedPlacement(deltaX, deltaY) {
  const element = selectedElement();
  if (!element) return false;
  const grid = element.position?.grid || roomTileSize();
  const position = placementPixelPosition(element);
  pushUndo();
  element.position = {
    grid,
    x: Math.round(position.x / grid) + deltaX,
    y: Math.round(position.y / grid) + deltaY,
  };
  delete element.x;
  delete element.y;
  updatePlacedSelectionInspector();
  drawRoom();
  setStatus(`Nudged the selected placement by ${grid}px.`);
  return true;
}

function bindEvents() {
  $("#scene-type").addEventListener("change", (event) => switchSceneType(event.target.value));
  $("#asset-search").addEventListener("input", filterAssets);
  $("#pack-filter").addEventListener("change", filterAssets);
  $("#refresh-catalog").addEventListener("click", refreshAssetCatalog);
  $("#layer").addEventListener("change", (event) => {
    if (state.tool === "select") {
      const element = selectedElement();
      if (!element) return;
      const nextLayer = event.target.value;
      const repairable = state.selectedPlaced.collection !== "placements";
      const template = repairable ? templateForRoom(element.template) : null;
      if (effectivePlacementLayer(element, template) === nextLayer) return;
      pushUndo();
      setPlacementLayer(element, repairable ? template : null, nextLayer);
      updatePlacedSelectionInspector();
      drawRoom();
      setStatus(`Moved the selected placement to the ${nextLayer} layer.`);
      return;
    }
    state.layer = event.target.value;
    updateSelectionDetails();
    drawRoom();
  });
  $("#placement-occludes-player").addEventListener("change", (event) => {
    const element = selectedElement();
    if (!element) return;
    pushUndo();
    setPlacementOcclusion(element, event.target.checked);
    updatePlacedSelectionInspector();
    setStatus(event.target.checked
      ? "The selected building component now fully hides the player when overlapping."
      : "The selected building component now uses the ordinary crown reveal.");
  });
  $("#behavior").addEventListener("change", drawRoom);
  $("#toggle-portable").addEventListener("click", toggleSelectedPortable);
  $("#portable-tool").addEventListener("change", (event) => updateSelectedPortableField("tool", event.target.value));
  $("#portable-label").addEventListener("change", (event) => updateSelectedPortableField("label", event.target.value.trim()));
  $("#portable-condition").addEventListener("change", (event) => updateSelectedPortableField("condition", event.target.value));
  $("#repair-view").addEventListener("change", (event) => {
    state.sceneRepairPreview = event.target.value;
    drawRoom();
    updatePlacedSelectionInspector();
    const label = state.sceneRepairPreview === "authored" ? "authored states" : `all ${state.sceneRepairPreview}`;
    setStatus(`Previewing ${label}; this display setting is not saved into the scene.`);
  });
  $("#capture-damaged").addEventListener("click", () => captureStateSource("damaged"));
  $("#capture-repaired").addEventListener("click", () => captureStateSource("repaired"));
  $("#repaired-invisible").addEventListener("change", updateStateSourceDetails);
  $("#pair-kind").addEventListener("change", (event) => populateTaskControls(defaultTaskForKind(slugify(event.target.value))));
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
    setStatus(`Stamp snap grid set to ${state.snapGrid}px; the ${state.collisionPen}px collision pen is independent.`);
  });
  $("#collision-pen").addEventListener("change", (event) => {
    state.collisionPen = Math.max(1, Math.min(256, Number(event.target.value) || 8));
    event.target.value = state.collisionPen;
    drawRoom();
    setStatus(`Collision pen set to ${state.collisionPen}px; existing collision areas are unchanged.`);
  });
  $("#toggle-grid").addEventListener("click", toggleGridVisibility);
  $("#toggle-collision").addEventListener("click", toggleCollisionVisibility);
  $("#flip-horizontal").addEventListener("click", () => toggleStampFlip("flip_x"));
  $("#flip-vertical").addEventListener("click", () => toggleStampFlip("flip_y"));
  $("#source-grid").addEventListener("change", () => {
    state.selection = null;
    drawSheet();
    updateSelectionDetails();
    drawRoom();
  });
  $("#smart-slice").addEventListener("click", toggleSmartSlice);
  $("#refresh-sheet").addEventListener("click", refreshSheetImage);
  $("#tool-group").addEventListener("click", (event) => {
    const button = event.target.closest("button[data-tool]"); if (!button) return;
    state.tool = button.dataset.tool;
    document.querySelectorAll("[data-tool]").forEach((item) => item.classList.toggle("active", item === button));
    $("#room-canvas").dataset.tool = state.tool;
    updateOrientationControls();
    updateLayerControl();
    drawRoom();
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
    state.room = freshRoom(state.sceneType);
    clearPlacedSelection();
    state.roomHoverPixel = null;
    state.undo = [];
    syncInputs(); drawRoom(); setStatus(`Started a new ${state.sceneType === "building" ? "building" : "interior"}.`);
  });
  $("#save-room").addEventListener("click", saveRoom);
  $("#load-room").addEventListener("click", loadRoom);
  $("#export-room").addEventListener("click", exportRoom);
  $("#undo").addEventListener("click", undo);
  $("#clear-placement-selection").addEventListener("click", () => {
    clearPlacedSelection();
    drawRoom();
    setStatus("Placement selection cleared.");
  });
  $("#placed-state-actions").addEventListener("click", (event) => {
    const button = event.target.closest("button[data-placement-preview]");
    if (!button || button.disabled || !selectedElement()) return;
    state.selectedRepairPreview = button.dataset.placementPreview;
    updatePlacedSelectionInspector();
    drawRoom();
    setStatus(`Selected item preview follows ${state.selectedRepairPreview === "scene" ? "the scene repair view" : `its ${state.selectedRepairPreview} state`}; saved state is unchanged.`);
  });

  const roomCanvas = $("#room-canvas");
  roomCanvas.dataset.tool = state.tool;
  roomCanvas.addEventListener("contextmenu", (event) => { event.preventDefault(); editRoom(roomCell(event), true); });
  roomCanvas.addEventListener("pointerenter", updateRoomHover);
  roomCanvas.addEventListener("pointerdown", (event) => { updateRoomHover(event); beginRoomStroke(event); });
  roomCanvas.addEventListener("pointermove", (event) => { updateRoomHover(event); continueRoomStroke(event); });
  roomCanvas.addEventListener("pointerup", finishRoomStroke);
  roomCanvas.addEventListener("pointercancel", finishRoomStroke);
  roomCanvas.addEventListener("pointerleave", clearRoomHover);

  const sheetCanvas = $("#sheet-canvas");
  sheetCanvas.addEventListener("pointerdown", (event) => {
    if (!state.sheetImage || selectSmartRegion(event)) return;
    state.sheetDrag = sheetCell(event);
    sheetCanvas.setPointerCapture(event.pointerId);
  });
  sheetCanvas.addEventListener("pointermove", (event) => { if (state.sheetDrag) drawSheet(rectangleFromCells(state.sheetDrag, sheetCell(event))); });
  sheetCanvas.addEventListener("pointerup", (event) => {
    if (!state.sheetDrag) return;
    state.selection = rectangleFromCells(state.sheetDrag, sheetCell(event)); state.sheetDrag = null;
    drawSheet(); updateSelectionDetails(); drawRoom();
  });
  window.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z") { event.preventDefault(); undo(); }
    if (event.target.closest?.("input, select, textarea")) return;
    const arrowDeltas = {
      ArrowLeft: [-1, 0],
      ArrowRight: [1, 0],
      ArrowUp: [0, -1],
      ArrowDown: [0, 1],
    };
    const delta = arrowDeltas[event.key];
    if (state.tool === "select" && delta && nudgeSelectedPlacement(...delta)) event.preventDefault();
    if (state.tool === "select" && event.key === "Escape" && state.selectedPlaced) {
      event.preventDefault();
      clearPlacedSelection();
      drawRoom();
    }
  });
}

async function initialize() {
  state.room = freshRoom(state.sceneType); syncInputs(); bindEvents(); updatePlacedSelectionInspector(); updateOrientationControls(); updateLayerControl(); drawRoom();
  const [catalogResponse, pairResponse] = await Promise.all([
    fetch("/api/catalog"),
    fetch("/api/repair-pairs"),
  ]);
  state.catalog = await catalogResponse.json();
  const pairDocument = await pairResponse.json();
  state.repairPairs = pairDocument.pairs || {};
  const packFilter = $("#pack-filter");
  populatePackFilter();
  if (state.sceneType === "building") {
    packFilter.value = state.catalog.packs.includes("components") ? "components" : "";
    $("#asset-search").value = "";
  } else {
    $("#asset-search").value = "motel";
  }
  filterAssets();
  renderRepairPairs();
  const firstPair = Object.keys(state.repairPairs).sort()[0];
  if (firstPair) selectRepairPair(firstPair);
  else newRepairPair();
  await refreshLevels();
  $("#refresh-catalog").disabled = false;
  setStatus(`Ready · ${state.catalog.count.toLocaleString()} private images indexed.`);
}

if (typeof window !== "undefined") {
  initialize().catch((error) => { console.error(error); setStatus(`Editor failed to start: ${error.message}`); });
}

if (typeof module !== "undefined") {
  module.exports = {
    applyBackgroundKey,
    assetUrl,
    collisionAreaBounds,
    collisionBrush,
    collisionCellsForRendering,
    collisionIndicesOverlapping,
    defaultTaskForKind,
    detectSmartRegions,
    draggedPlacementPosition,
    drawTransformedImage,
    footprintForPixelSizes,
    gridLine,
    invalidateAssetImage,
    paintPairPreview,
    findPlacedItemAtPixel,
    effectivePlacementLayer,
    placementPixelPosition,
    repairStateForElement,
    setPlacementLayer,
    setPlacementOcclusion,
    repairPairMatches,
    roomGridLayersForRendering,
    snapCellForPixel,
    stampAnchorForUnit,
    stampPreviewPlacement,
    stampUnitForCell,
    subtractCollisionArea,
  };
}
