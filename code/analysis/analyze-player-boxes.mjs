import fs from "node:fs";
import { decompressSave } from "./inspect-save.mjs";
import { scanEmbeddedItems } from "./parse-gvas.mjs";

const savePath = process.argv[2];
if (!savePath) throw new Error("Usage: node analyze-player-boxes.mjs <save.sav>");

const raw = decompressSave(fs.readFileSync(savePath));
const items = scanEmbeddedItems(raw);

function readFString(offset) {
  const length = raw.readInt32LE(offset);
  if (length <= 0 || length > 4096 || offset + 4 + length > raw.length) return null;
  const value = raw.subarray(offset + 4, offset + 3 + length).toString("utf8");
  return { value, next: offset + 4 + length };
}

const classRecords = [];
const needle = Buffer.from("/Game/Blueprints/", "utf8");
let cursor = 0;
while ((cursor = raw.indexOf(needle, cursor)) !== -1) {
  const lengthOffset = cursor - 4;
  const string = lengthOffset >= 0 ? readFString(lengthOffset) : null;
  if (string?.value.startsWith("/Game/Blueprints/") && string.value.includes("_C")) {
    const actor = readFString(string.next);
    if (actor?.value && actor.value.length < 512) {
      classRecords.push({ offset: lengthOffset, classPath: string.value, actorName: actor.value });
    }
  }
  cursor += needle.length;
}

const boxes = classRecords
  .map((record, index) => ({ ...record, end: classRecords[index + 1]?.offset ?? raw.length }))
  .filter(({ classPath }) => /BP_Inventory_PlayerBox(?:_Small)?\./.test(classPath))
  .map((record) => ({
    ...record,
    bytes: record.end - record.offset,
    growthItems: items
      .filter(({ offset, assetName }) => offset >= record.offset && offset < record.end && /^DA_(?:Food|Memory)_/.test(assetName))
      .map(({ offset, assetName, itemCount }) => ({ offset, assetName, itemCount })),
  }));

console.log(JSON.stringify({ classRecordCount: classRecords.length, playerBoxCount: boxes.length, boxes }, null, 2));
