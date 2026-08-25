import fs from "node:fs";
import { decompressSave, printableStrings } from "./inspect-save.mjs";
import { parseGvas, scanEmbeddedItems } from "./parse-gvas.mjs";

const savePath = process.argv[2];
if (!savePath) {
  console.error("Usage: node analyze-backpack.mjs <save.sav>");
  process.exit(2);
}

const raw = decompressSave(fs.readFileSync(savePath));
const playerStart = raw.indexOf(Buffer.from("/Game/Blueprints/BP_FirstPersonCharacter_New.BP_FirstPersonCharacter_New_C"));
const playerEnd = raw.indexOf(Buffer.from("PersistentLevel.BP_FirstPersonCharacter_New_C1"), playerStart);
if (playerStart < 0 || playerEnd < 0) throw new Error("Player actor range was not found.");

const orderedItems = scanEmbeddedItems(raw)
  .filter(({ offset, assetName }) => offset >= playerStart && offset < playerEnd && /^DA_(?:Food|Memory)_/.test(assetName))
  .map(({ offset, assetName, itemCount }) => ({ offset, offsetHex: `0x${offset.toString(16)}`, assetName, itemCount }));

function repeatedSuffix(items) {
  for (let length = Math.floor(items.length / 2); length >= 1; length -= 1) {
    const firstStart = items.length - length * 2;
    const first = items.slice(firstStart, firstStart + length);
    const second = items.slice(firstStart + length);
    if (first.every((item, index) => item.assetName === second[index].assetName && item.itemCount === second[index].itemCount)) {
      return { prefixLength: firstStart, repeatedLength: length, first, second };
    }
  }
  return null;
}

function repeatedPrefix(items) {
  for (let length = Math.floor(items.length / 2); length >= 1; length -= 1) {
    const first = items.slice(0, length);
    const second = items.slice(length, length * 2);
    if (first.every((item, index) => item.assetName === second[index].assetName && item.itemCount === second[index].itemCount)) {
      return { repeatedLength: length, trailingLength: items.length - length * 2, first, second };
    }
  }
  return null;
}

const parsed = parseGvas(raw);
const structuredHits = parsed.hits
  .filter(({ offset, assetName }) => offset >= playerStart && offset < playerEnd && /^DA_(?:Food|Memory)_/.test(assetName))
  .map(({ offset, path, assetName, itemCount }) => ({ offset, offsetHex: `0x${offset.toString(16)}`, path, assetName, itemCount }));

function nearbyInventoryStrings(offset) {
  const start = Math.max(playerStart, offset - 4096);
  const end = Math.min(playerEnd, offset + 256);
  return printableStrings(raw.subarray(start, end))
    .map((entry) => ({ ...entry, offset: entry.offset + start }))
    .filter(({ value }) => /item|inventory|backpack|component/i.test(value))
    .map(({ offset: stringOffset, value }) => ({ offset: stringOffset, offsetHex: `0x${stringOffset.toString(16)}`, value }));
}

console.log(JSON.stringify({
  rawBytes: raw.length,
  playerRange: { start: playerStart, end: playerEnd },
  orderedItems,
  repeatedPrefix: repeatedPrefix(orderedItems),
  repeatedSuffix: repeatedSuffix(orderedItems),
  structuredHits,
  playerInventoryStrings: printableStrings(raw.subarray(playerStart, playerEnd))
    .map((entry) => ({ ...entry, offset: entry.offset + playerStart }))
    .filter(({ value }) => /backpack|inventory|equipment|quick|slot|storage/i.test(value))
    .map(({ offset: stringOffset, value }) => ({ offset: stringOffset, offsetHex: `0x${stringOffset.toString(16)}`, value })),
  firstSequenceContext: orderedItems[0] ? nearbyInventoryStrings(orderedItems[0].offset) : [],
  secondSequenceContext: orderedItems[11] ? nearbyInventoryStrings(orderedItems[11].offset) : [],
  trailingSequenceContext: orderedItems[22] ? nearbyInventoryStrings(orderedItems[22].offset) : [],
}, null, 2));
