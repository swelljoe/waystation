"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const editorPath = path.join(__dirname, "..", "tools", "level-editor", "editor.js");
const indexPath = path.join(__dirname, "..", "tools", "level-editor", "index.html");
const { collisionCellsForRendering } = require(editorPath);

const room = { collision: [{ x: 2, y: 3 }, { x: 5, y: 1 }] };
const original = structuredClone(room.collision);

assert.equal(collisionCellsForRendering(room, true), room.collision);
assert.deepEqual(collisionCellsForRendering(room, false), []);
assert.deepEqual(room.collision, original, "hiding the overlay must not mutate collision data");

const index = fs.readFileSync(indexPath, "utf8");
assert.match(index, /id="toggle-collision"/);
assert.match(index, /aria-pressed="true"/);

console.log("level editor UI tests passed");
