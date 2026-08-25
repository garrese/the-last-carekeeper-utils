import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import zlib from "node:zlib";

const WRAPPER_MAGIC = Buffer.from([0xc1, 0x83, 0x2a, 0x9e, 0x22, 0x22, 0x22, 0x22]);
const HEADER_SIZE = 49;

function readUInt24LE(buffer, offset) {
  return buffer[offset] | (buffer[offset + 1] << 8) | (buffer[offset + 2] << 16);
}

export function decompressSave(buffer) {
  const chunks = [];
  let offset = 0;

  while (offset < buffer.length) {
    if (offset + HEADER_SIZE > buffer.length || !buffer.subarray(offset, offset + 8).equals(WRAPPER_MAGIC)) {
      throw new Error(`Cabecera de bloque no reconocida en 0x${offset.toString(16)}`);
    }

    const compressedSize = readUInt24LE(buffer, offset + 17);
    const uncompressedSize = readUInt24LE(buffer, offset + 25);
    const compressed = buffer.subarray(offset + HEADER_SIZE, offset + HEADER_SIZE + compressedSize);
    const raw = zlib.inflateSync(compressed);

    if (raw.length !== uncompressedSize) {
      throw new Error(`Incorrect size at 0x${offset.toString(16)}: ${raw.length} != ${uncompressedSize}`);
    }

    chunks.push(raw);
    offset += HEADER_SIZE + compressedSize;
  }

  return Buffer.concat(chunks);
}

export function printableStrings(buffer, minLength = 4) {
  const results = [];
  let start = -1;

  for (let i = 0; i <= buffer.length; i += 1) {
    const byte = i < buffer.length ? buffer[i] : 0;
    if (byte >= 0x20 && byte <= 0x7e) {
      if (start === -1) start = i;
    } else if (start !== -1) {
      if (i - start >= minLength) {
        results.push({ offset: start, value: buffer.toString("utf8", start, i) });
      }
      start = -1;
    }
  }

  return results;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const savePath = process.argv[2];
  const filter = process.argv[3] ? new RegExp(process.argv[3], "i") : /inventory|container|item|memory|food/i;
  if (!savePath) {
    console.error("Usage: node inspect-save.mjs <save.sav> [regex-filter]");
    process.exit(2);
  }

  const compressed = fs.readFileSync(savePath);
  const raw = decompressSave(compressed);
  console.log(JSON.stringify({ compressedBytes: compressed.length, rawBytes: raw.length }));

  for (const item of printableStrings(raw).filter(({ value }) => filter.test(value))) {
    console.log(`${item.offset.toString(16).padStart(8, "0")}  ${item.value}`);
  }
}
