import fs from "node:fs";
import { decompressSave } from "./inspect-save.mjs";

const [savePath, offsetText = "0", lengthText = "512"] = process.argv.slice(2);
const offset = Number.parseInt(offsetText, offsetText.startsWith("0x") ? 16 : 10);
const length = Number.parseInt(lengthText, lengthText.startsWith("0x") ? 16 : 10);
const raw = decompressSave(fs.readFileSync(savePath));

for (let row = 0; row < length; row += 16) {
  const start = offset + row;
  const bytes = raw.subarray(start, Math.min(start + 16, offset + length));
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join(" ").padEnd(47);
  const text = [...bytes].map((byte) => (byte >= 0x20 && byte <= 0x7e ? String.fromCharCode(byte) : ".")).join("");
  console.log(`${start.toString(16).padStart(8, "0")}  ${hex}  ${text}`);
}
