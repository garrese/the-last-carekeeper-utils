import fs from "node:fs";
import { decompressSave } from "./inspect-save.mjs";
import { scanEmbeddedItems } from "./parse-gvas.mjs";

const savePath = process.argv[2];
const raw = decompressSave(fs.readFileSync(savePath));
const items = scanEmbeddedItems(raw).filter(({ assetName }) => /^DA_(?:Food|Memory)_/.test(assetName));
const playerStart = raw.indexOf(Buffer.from("/Game/Blueprints/BP_FirstPersonCharacter_New.BP_FirstPersonCharacter_New_C"));
const playerEnd = raw.indexOf(Buffer.from("PersistentLevel.BP_FirstPersonCharacter_New_C1"), playerStart);

function aggregate(list) {
  const totals = new Map();
  for (const item of list) {
    const current = totals.get(item.assetName) ?? { occurrences: 0, count: 0 };
    current.occurrences += 1;
    current.count += item.itemCount ?? 0;
    totals.set(item.assetName, current);
  }
  return Object.fromEntries([...totals.entries()].sort(([left], [right]) => left.localeCompare(right)));
}

console.log(JSON.stringify({
  rawBytes: raw.length,
  playerRange: { start: playerStart, end: playerEnd },
  player: aggregate(items.filter(({ offset }) => offset >= playerStart && offset < playerEnd)),
  world: aggregate(items),
}, null, 2));
