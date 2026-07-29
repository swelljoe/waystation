"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const editorPath = path.join(__dirname, "..", "tools", "level-editor", "editor.js");
const indexPath = path.join(__dirname, "..", "tools", "level-editor", "index.html");
const {
  applyBackgroundKey,
  assetUrl,
  collisionCellsForRendering,
  detectSmartRegions,
  draggedPlacementPosition,
  effectivePlacementLayer,
  drawTransformedImage,
  footprintForPixelSizes,
  findPlacedItemAtPixel,
  gridLine,
  invalidateAssetImage,
  paintPairPreview,
  placementPixelPosition,
  repairStateForElement,
  setPlacementLayer,
  repairPairMatches,
  snapCellForPixel,
  stampAnchorForUnit,
  stampPreviewPlacement,
  stampUnitForCell,
} = require(editorPath);

const refreshedPath = "components/Village House/TX House.png";
assert.equal(assetUrl(refreshedPath), "/asset/components/Village%20House/TX%20House.png");
assert.equal(invalidateAssetImage(refreshedPath), 1);
assert.equal(
  assetUrl(refreshedPath),
  "/asset/components/Village%20House/TX%20House.png?revision=1",
);
assert.equal(invalidateAssetImage(refreshedPath), 2);
assert.match(assetUrl(refreshedPath), /revision=2$/);

function pixelData(width, height, color = [0, 0, 0, 0]) {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let index = 0; index < width * height; index++) data.set(color, index * 4);
  return { width, height, data };
}

function setPixel(image, x, y, color) {
  image.data.set(color, (y * image.width + x) * 4);
}

const transparentSheet = pixelData(8, 3);
for (let y = 0; y < 3; y++) {
  for (let x = 0; x < 3; x++) setPixel(transparentSheet, x, y, [80, 50, 20, 255]);
  for (let x = 5; x < 8; x++) setPixel(transparentSheet, x, y, [90, 60, 30, 255]);
}
assert.deepEqual(
  detectSmartRegions(transparentSheet).regions.map(({ x, y, width, height }) => ({ x, y, width, height })),
  [{ x: 0, y: 0, width: 3, height: 3 }, { x: 5, y: 0, width: 3, height: 3 }],
);

const solidBackgroundSheet = pixelData(7, 3, [253, 253, 253, 255]);
for (let y = 0; y < 3; y++) {
  for (let x = 2; x < 5; x++) setPixel(solidBackgroundSheet, x, y, [50, 40, 30, 255]);
}
const solidDetection = detectSmartRegions(solidBackgroundSheet);
assert.deepEqual(solidDetection.backgroundKey.color, [253, 253, 253]);
assert.deepEqual(
  solidDetection.regions.map(({ x, y, width, height }) => ({ x, y, width, height })),
  [{ x: 2, y: 0, width: 3, height: 3 }],
);
applyBackgroundKey(solidBackgroundSheet, solidDetection.backgroundKey);
assert.equal(solidBackgroundSheet.data[3], 0);
assert.equal(solidBackgroundSheet.data[(2 * 4) + 3], 255);

const room = { collision: [{ x: 2, y: 3 }, { x: 5, y: 1 }] };
const original = structuredClone(room.collision);

assert.equal(collisionCellsForRendering(room, true), room.collision);
assert.deepEqual(collisionCellsForRendering(room, false), []);
assert.deepEqual(room.collision, original, "hiding the overlay must not mutate collision data");
assert.deepEqual(
  placementPixelPosition({ position: { grid: 16, x: 3, y: -1 } }, 32),
  { x: 48, y: -16 },
);
assert.deepEqual(placementPixelPosition({ x: 2, y: 3 }, 32), { x: 64, y: 96 });
assert.deepEqual(snapCellForPixel({ x: 47.9, y: 32 }, 16), { x: 2, y: 2 });
assert.equal(snapCellForPixel(null, 16), null);
assert.deepEqual(
  draggedPlacementPosition({ x: 53, y: 3 }, { x: 9, y: 15 }, 16),
  { grid: 16, x: 3, y: -1 },
);
assert.equal(repairStateForElement({ initial_state: "damaged" }, "authored", "scene", false), "damaged");
assert.equal(repairStateForElement({ initial_state: "damaged" }, "repaired", "scene", false), "repaired");
assert.equal(repairStateForElement({ initial_state: "damaged" }, "repaired", "damaged", true), "damaged");
assert.equal(effectivePlacementLayer({ layer: "overlay" }, { layer: "wall" }), "overlay");
assert.equal(effectivePlacementLayer({}, { layer: "wall" }), "wall");
const repairableLayer = {};
setPlacementLayer(repairableLayer, { layer: "wall" }, "overlay");
assert.deepEqual(repairableLayer, { layer: "overlay" });
setPlacementLayer(repairableLayer, { layer: "wall" }, "wall");
assert.deepEqual(repairableLayer, {}, "returning to the pair default should remove the override");
const bakedLayer = { layer: "floor" };
setPlacementLayer(bakedLayer, null, "object");
assert.deepEqual(bakedLayer, { layer: "object" });
const overlappingRenderables = [
  {
    placement: { position: { grid: 16, x: 1, y: 1 }, source: { grid: 1, width: 32, height: 32 } },
    collection: "placements",
    index: 3,
  },
  {
    placement: { position: { grid: 16, x: 1, y: 1 }, source: { grid: 1, width: 16, height: 16 } },
    collection: "fixtures",
    index: 2,
  },
];
assert.deepEqual(findPlacedItemAtPixel({ x: 20, y: 20 }, overlappingRenderables, 32), { collection: "fixtures", index: 2 });
assert.deepEqual(findPlacedItemAtPixel({ x: 40, y: 40 }, overlappingRenderables, 32), { collection: "placements", index: 3 });
assert.equal(findPlacedItemAtPixel({ x: 4, y: 4 }, overlappingRenderables, 32), null);
assert.equal(
  stampPreviewPlacement({
    behavior: "baked",
    selection: null,
    layer: "floor",
    template: null,
    transform: {},
    snapGrid: 16,
    cell: { x: 0, y: 0 },
  }),
  null,
);

const keyedSelection = {
  path: "components/damaged-house.png",
  grid: 1,
  x: 12,
  y: 18,
  width: 37,
  height: 29,
  background_key: { color: [253, 253, 253], tolerance: 24, softness: 16 },
};
assert.deepEqual(
  stampPreviewPlacement({
    behavior: "baked",
    selection: keyedSelection,
    layer: "overlay",
    template: null,
    transform: { flip_x: true, flip_y: false },
    snapGrid: 16,
    cell: { x: 3, y: 5 },
  }),
  {
    layer: "overlay",
    position: { grid: 16, x: 3, y: 5 },
    source: keyedSelection,
    transform: { flip_x: true, flip_y: false },
  },
);
const damagedSource = { path: "components/house.png", grid: 1, x: 2, y: 4, width: 20, height: 30 };
assert.deepEqual(
  stampPreviewPlacement({
    behavior: "structure",
    selection: null,
    layer: "floor",
    template: { layer: "wall", states: { damaged: { source: damagedSource } } },
    transform: {},
    snapGrid: 32,
    cell: { x: 1, y: 2 },
  }),
  {
    layer: "wall",
    position: { grid: 32, x: 1, y: 2 },
    source: damagedSource,
  },
);

const footprint = footprintForPixelSizes(
  [{ width: 32, height: 48 }, { width: 70, height: 64 }],
  32,
);
assert.deepEqual(footprint, { width: 3, height: 2 });
const stampOrigin = { x: 4, y: 4 };
assert.deepEqual(stampUnitForCell(stampOrigin, { x: 6, y: 5 }, footprint), { x: 0, y: 0 });
assert.deepEqual(stampUnitForCell(stampOrigin, { x: 7, y: 5 }, footprint), { x: 1, y: 0 });
assert.deepEqual(stampUnitForCell(stampOrigin, { x: 3, y: 4 }, footprint), { x: -1, y: 0 });
assert.deepEqual(
  stampAnchorForUnit(stampOrigin, { x: 1, y: -1 }, footprint),
  { x: 7, y: 2 },
);
assert.deepEqual(
  gridLine({ x: 0, y: 0 }, { x: 3, y: 1 }),
  [{ x: 0, y: 0 }, { x: 1, y: 0 }, { x: 2, y: 1 }, { x: 3, y: 1 }],
);

const sharedRepaired = { source: { path: "hotel/wall.png", x: 1 } };
const wallA = {
  label: "Cracked plaster A",
  kind: "plaster",
  layer: "wall",
  states: { damaged: { source: { path: "hotel/cracks.png", x: 2 } }, repaired: sharedRepaired },
};
const wallB = {
  label: "Cracked plaster B",
  kind: "plaster",
  layer: "wall",
  states: { damaged: { source: { path: "hotel/cracks.png", x: 3 } }, repaired: sharedRepaired },
};
assert.equal(repairPairMatches("plaster-a", wallA, "cracked wall"), true);
assert.equal(repairPairMatches("plaster-b", wallB, "plaster-b"), true);
assert.notEqual(wallA, wallB, "pairs sharing a crop still have independent records");

let previewDraw = null;
const previewOperations = [];
const detachedCanvas = {
  isConnected: false,
  width: 38,
  height: 38,
  getContext: () => ({
    save: () => previewOperations.push("save"),
    translate: (...arguments_) => previewOperations.push(["translate", ...arguments_]),
    scale: (...arguments_) => previewOperations.push(["scale", ...arguments_]),
    drawImage: (...arguments_) => { previewDraw = arguments_; },
    restore: () => previewOperations.push("restore"),
  }),
};
const cachedImage = { naturalWidth: 128, naturalHeight: 128 };
const previewSource = { grid: 16, x: 1, y: 2, width: 2, height: 1 };
assert.equal(paintPairPreview(detachedCanvas, cachedImage, previewSource), true);
assert.deepEqual(previewDraw.slice(1), [16, 32, 32, 16, 0, 0, 38, 19]);
assert.deepEqual(previewOperations, ["save", ["translate", 0, 9], ["scale", 1, 1], "restore"]);

previewOperations.length = 0;
paintPairPreview(detachedCanvas, cachedImage, previewSource, { flip_x: true, flip_y: true });
assert.deepEqual(previewOperations, ["save", ["translate", 38, 28], ["scale", -1, -1], "restore"]);
assert.equal(typeof drawTransformedImage, "function");

const index = fs.readFileSync(indexPath, "utf8");
assert.match(index, /id="toggle-collision"/);
assert.match(index, /aria-pressed="true"/);
assert.match(index, /id="repair-pair-library"/);
assert.match(index, /id="pair-list"/);
assert.match(index, /id="duplicate-pair"/);
assert.match(index, /id="flip-horizontal"/);
assert.match(index, /id="flip-vertical"/);
assert.match(index, /id="snap-grid"/);
assert.match(index, /id="scene-type"/);
assert.match(index, /id="smart-slice"/);
assert.match(index, /id="refresh-sheet"/);
assert.match(index, /data-tool="select"/);
assert.match(index, /id="repair-view"/);
assert.match(index, /id="placed-selection-card"/);
assert.match(index, /data-placement-preview="repaired"/);
assert.match(index, /Floor \(bottom\)[\s\S]*Wall[\s\S]*Object[\s\S]*Overlay \(top\)/);

console.log("level editor UI tests passed");
