"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const editorPath = path.join(__dirname, "..", "tools", "level-editor", "editor.js");
const indexPath = path.join(__dirname, "..", "tools", "level-editor", "index.html");
const {
  collisionCellsForRendering,
  drawTransformedImage,
  footprintForPixelSizes,
  gridLine,
  paintPairPreview,
  placementPixelPosition,
  repairPairMatches,
  stampAnchorForUnit,
  stampUnitForCell,
} = require(editorPath);

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

console.log("level editor UI tests passed");
