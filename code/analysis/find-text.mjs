import fs from "node:fs";
import { decompressSave } from "./inspect-save.mjs";

const [savePath, query] = process.argv.slice(2);
const raw = decompressSave(fs.readFileSync(savePath));
for (const encoding of ["utf8", "utf16le"]) {
  const needle = Buffer.from(query, encoding);
  let offset = 0;
  let found = false;
  while ((offset = raw.indexOf(needle, offset)) !== -1) {
    console.log(`${encoding}  0x${offset.toString(16)}  ${offset}`);
    found = true;
    offset += needle.length;
  }
  if (!found) console.log(`${encoding}  no encontrado`);
}
