import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { decompressSave } from "./inspect-save.mjs";

class Reader {
  constructor(buffer, offset = 0) {
    this.buffer = buffer;
    this.offset = offset;
  }

  ensure(size) {
    if (this.offset + size > this.buffer.length) {
      throw new Error(`Read outside the file at 0x${this.offset.toString(16)}`);
    }
  }

  uint8() { this.ensure(1); return this.buffer[this.offset++]; }
  int32() { this.ensure(4); const value = this.buffer.readInt32LE(this.offset); this.offset += 4; return value; }
  uint32() { this.ensure(4); const value = this.buffer.readUInt32LE(this.offset); this.offset += 4; return value; }
  int64() { this.ensure(8); const value = this.buffer.readBigInt64LE(this.offset); this.offset += 8; return value; }
  uint64() { this.ensure(8); const value = this.buffer.readBigUInt64LE(this.offset); this.offset += 8; return value; }
  float() { this.ensure(4); const value = this.buffer.readFloatLE(this.offset); this.offset += 4; return value; }
  double() { this.ensure(8); const value = this.buffer.readDoubleLE(this.offset); this.offset += 8; return value; }
  bytes(size) { this.ensure(size); const value = this.buffer.subarray(this.offset, this.offset + size); this.offset += size; return value; }
  skip(size) { this.ensure(size); this.offset += size; }

  string() {
    const length = this.int32();
    if (length === 0) return "";
    if (length > 0) {
      const bytes = this.bytes(length);
      return bytes.subarray(0, Math.max(0, length - 1)).toString("utf8");
    }
    const charCount = -length;
    const bytes = this.bytes(charCount * 2);
    return bytes.subarray(0, Math.max(0, bytes.length - 2)).toString("utf16le");
  }
}

function readTypeName(reader, depth = 0) {
  if (depth > 12) throw new Error("Property type is nested too deeply");
  const name = reader.string();
  const parameterCount = reader.int32();
  if (parameterCount < 0 || parameterCount > 12) {
    throw new Error(`Invalid parameter count (${parameterCount}) for ${name}`);
  }
  return { name, parameters: Array.from({ length: parameterCount }, () => readTypeName(reader, depth + 1)) };
}

function isTaggedStruct(typeName) {
  const structName = typeName.parameters[0]?.name ?? "";
  const owner = typeName.parameters[0]?.parameters[0]?.name ?? "";
  const rawStructs = new Set([
    "Vector", "Vector2D", "Vector4", "Quat", "Rotator", "Transform", "Guid", "LinearColor", "Color",
    "DateTime", "Timespan", "IntPoint", "IntVector", "Box", "Box2D", "SoftObjectPath", "GameplayTag"
  ]);
  return !rawStructs.has(structName) && (owner.startsWith("/Script/") || structName.startsWith("Voyage") || structName === "PrimaryAssetType");
}

function readPropertyList(reader, end, path, hits, depth = 0) {
  const properties = [];
  while (reader.offset < end) {
    const propertyOffset = reader.offset;
    const name = reader.string();
    if (name === "None") break;
    let type;
    try {
      type = readTypeName(reader);
    } catch (error) {
      throw new Error(`Could not read the type of ${path}.${name} at 0x${propertyOffset.toString(16)} (end 0x${end.toString(16)}): ${error.message}`);
    }
    const size = reader.uint32();
    let boolValue;
    if (type.name === "BoolProperty") boolValue = reader.uint8() !== 0;
    const hasGuid = reader.uint8();
    if (hasGuid) reader.skip(16);
    const dataStart = reader.offset;
    const dataEnd = dataStart + size;
    if (dataEnd > end) {
      throw new Error(`Property ${path}.${name} (0x${propertyOffset.toString(16)}) exceeds its container`);
    }

    let value;
    try {
      value = type.name === "BoolProperty" ? boolValue : readValue(reader, type, dataEnd, `${path}.${name}`, hits, depth + 1);
    } catch (error) {
      value = { skipped: true, error: error.message };
      reader.offset = dataEnd;
    }
    if (reader.offset !== dataEnd) reader.offset = dataEnd;
    const property = { name, type, value, offset: propertyOffset, size };
    properties.push(property);
  }

  const byName = Object.fromEntries(properties.map((property) => [property.name, property.value]));
  if (typeof byName.AssetName === "string" && byName.AssetName.startsWith("DA_")) {
    const itemCount = byName.Data?.byName?.ItemCount;
    hits.push({ path, assetName: byName.AssetName, itemCount: typeof itemCount === "number" ? itemCount : null, offset: properties[0]?.offset ?? reader.offset });
  }
  return { properties, byName };
}

function readMapKeyOrValue(reader, type, end, path, hits, depth) {
  return readValue(reader, type, end, path, hits, depth);
}

function readValue(reader, type, end, path, hits, depth) {
  if (depth > 100) throw new Error("Structure is nested too deeply");
  switch (type.name) {
    case "IntProperty": return reader.int32();
    case "UInt32Property": return reader.uint32();
    case "Int64Property": return Number(reader.int64());
    case "UInt64Property": return Number(reader.uint64());
    case "FloatProperty": return reader.float();
    case "DoubleProperty": return reader.double();
    case "StrProperty":
    case "NameProperty":
    case "TextProperty": return reader.string();
    case "EnumProperty": return reader.string();
    case "ByteProperty": {
      if (end - reader.offset === 1) return reader.uint8();
      if (end - reader.offset >= 4) return reader.string();
      return reader.bytes(end - reader.offset);
    }
    case "ObjectProperty":
    case "SoftObjectProperty":
    case "ClassProperty": return reader.string();
    case "StructProperty": {
      if (isTaggedStruct(type)) return readPropertyList(reader, end, path, hits, depth + 1);
      return { raw: reader.bytes(end - reader.offset) };
    }
    case "ArrayProperty": {
      const count = reader.int32();
      const inner = type.parameters[0];
      if (!inner || count < 0 || count > 10_000_000) throw new Error(`Invalid array at ${path}`);
      if (inner.name === "ByteProperty") return { count, raw: reader.bytes(end - reader.offset) };
      const values = [];
      for (let index = 0; index < count; index += 1) {
        values.push(readMapKeyOrValue(reader, inner, end, `${path}[${index}]`, hits, depth + 1));
      }
      return values;
    }
    case "MapProperty": {
      const removedCount = reader.int32();
      const count = reader.int32();
      const [keyType, valueType] = type.parameters;
      if (!keyType || !valueType || removedCount < 0 || count < 0 || count > 10_000_000) {
        throw new Error(`Invalid map at ${path}`);
      }
      const entries = [];
      for (let index = 0; index < count; index += 1) {
        const key = readMapKeyOrValue(reader, keyType, end, `${path}{key:${index}}`, hits, depth + 1);
        const value = readMapKeyOrValue(reader, valueType, end, `${path}{${String(key)}}`, hits, depth + 1);
        entries.push({ key, value });
      }
      return { removedCount, entries };
    }
    case "SetProperty": {
      const removedCount = reader.int32();
      const count = reader.int32();
      const inner = type.parameters[0];
      const values = [];
      for (let index = 0; index < count; index += 1) values.push(readValue(reader, inner, end, `${path}{${index}}`, hits, depth + 1));
      return { removedCount, values };
    }
    default:
      return { raw: reader.bytes(end - reader.offset), unknownType: type.name };
  }
}

export function parseGvas(raw) {
  const reader = new Reader(raw, 4);
  if (reader.bytes(4).toString("ascii") !== "GVAS") throw new Error("GVAS header was not found");
  const saveGameVersion = reader.int32();
  const packageVersion = reader.int32();
  const packageVersionUE5 = reader.int32();
  const engine = {
    major: reader.buffer.readUInt16LE(reader.offset),
    minor: reader.buffer.readUInt16LE(reader.offset + 2),
    patch: reader.buffer.readUInt16LE(reader.offset + 4),
  };
  reader.skip(6);
  engine.changelist = reader.uint32();
  engine.branch = reader.string();
  const customVersionFormat = reader.int32();
  const customVersionCount = reader.int32();
  reader.skip(customVersionCount * 20);
  const saveGameClass = reader.string();
  // UE 5.4+ serializes the unversioned-properties flag here.
  const usesUnversionedProperties = reader.uint8() !== 0;
  const hits = [];
  const root = readPropertyList(reader, raw.length, "$", hits);
  return { header: { saveGameVersion, packageVersion, packageVersionUE5, engine, customVersionFormat, customVersionCount, saveGameClass, usesUnversionedProperties }, root, hits };
}

export function scanEmbeddedItems(raw) {
  const marker = Buffer.from([0x0a, 0x00, 0x00, 0x00, ...Buffer.from("AssetName\0", "ascii")]);
  const items = [];
  let offset = 0;
  while ((offset = raw.indexOf(marker, offset)) !== -1) {
    const reader = new Reader(raw, offset);
    const hits = [];
    try {
      const parsed = readPropertyList(reader, Math.min(raw.length, offset + 1024 * 1024), `@0x${offset.toString(16)}`, hits);
      const assetName = parsed.byName.AssetName;
      const itemCount = parsed.byName.Data?.byName?.ItemCount;
      if (typeof assetName === "string" && assetName.startsWith("DA_")) {
        items.push({
          offset,
          assetName,
          itemCount: typeof itemCount === "number" ? itemCount : null,
          data: parsed.byName.Data,
        });
      }
    } catch {
      // Other fields named AssetName are not VoyageItemSerialize records.
    }
    offset += marker.length;
  }
  return items;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const savePath = process.argv[2];
  const raw = decompressSave(fs.readFileSync(savePath));
  const parsed = parseGvas(raw);
  const embeddedItems = scanEmbeddedItems(raw);
  const growthItems = embeddedItems.filter(({ assetName }) => /^DA_(?:Food|Memory)_/.test(assetName));
  const rootProperties = parsed.root.properties.map(({ name, type, size, value }) => ({
    name,
    type: type.name,
    size,
    skipped: value?.skipped ?? false,
    error: value?.error,
  }));
  console.log(JSON.stringify({ header: parsed.header, rootProperties, hitCount: parsed.hits.length, embeddedItemCount: embeddedItems.length, firstHits: parsed.hits.slice(0, 20), growthItems }, null, 2));
}
