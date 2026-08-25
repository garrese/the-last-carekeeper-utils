export type ImportedItem = {
  assetName: string;
  mappedName: string | null;
  quantity: number;
};

export type InventorySource = {
  id: string;
  label: string;
  kind: 'backpack' | 'player-box';
  items: ImportedItem[];
};

export type SaveImportResult = {
  sources: InventorySource[];
  customLabelFound: boolean;
  rawBytes: number;
  blockCount: number;
};

type TypeName = { name: string; parameters: TypeName[] };

const MAGIC = new Uint8Array([0xc1, 0x83, 0x2a, 0x9e, 0x22, 0x22, 0x22, 0x22]);
const HEADER_SIZE = 49;
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

const ASSET_ALIASES: Record<string, string> = {
  'DA_Food_High-FatEnergy': 'High-Fat',
  'DA_Food_Mind_Surge': 'Mind Surge',
  'DA_Food_Nutri-Core': 'Nutri-Core',
  'DA_Food_PhysiqueFuel': 'Physique Fuel',
  'DA_Food_Bone-Fortify': 'Bone-Fortify',
  'DA_Food_Endura-Growth': 'Endura-Growth',
  'DA_Food_ImmuneBoost': 'Immune Boost',
  'DA_Food_MuscleFortification': 'Muscle Fortification',
  'DA_Food_Neuro-Boost': 'Neuro-Boost',
  'DA_Food_Hyper-Evolution': 'Hyper-Evolution',
  'DA_Food_MitochondrialSurge': 'Mitochondrial Surge',
  'DA_Food_NaniteInfusion': 'Nanite Infusion',
  'DA_Food_UltimateGenesis': 'Ultimate Genesis',
  'DA_Food_Pear': 'Pear',
  'DA_Memory_BasketBall': 'Basketball',
  'DA_Memory_BasketBall_Blue': 'Basketball',
  'DA_Memory_Books_Encyclopedia': 'Encyclopedia',
  'DA_Memory_Books_FirstAid': 'First Aid',
  'DA_Memory_Books_Meditation': 'Meditation',
  'DA_Memory_Books_Programming': 'Programming Manual',
  'DA_Memory_Books_Sudoku': 'Sudoku Book',
  'DA_Memory_Books_SunTzu': 'The Art of War',
  'DA_Memory_Books_Tommy': 'Tommy',
  'DA_Memory_BowlingBall': 'Bowling Ball',
  'DA_Memory_BowlingPin': 'Bowling Pin',
  'DA_Memory_Camera': 'Camera',
  'DA_Memory_Compass': 'Compass',
  'DA_Memory_Crayon': 'Crayon',
  'DA_Memory_Drawings_Biology': 'Biology Notes',
  'DA_Memory_Drawings_Blueprints': 'Blueprints',
  'DA_Memory_Drawings_Cards': 'Cards',
  'DA_Memory_Drawings_Diagrams': 'Assembly Instructions',
  'DA_Memory_Drawings_Kids': 'Small Human Art',
  'DA_Memory_Drawings_Letters': 'Love Letters',
  'DA_Memory_Drawings_Logs': "Commander's Log",
  'DA_Memory_Drawings_Maps': 'Maps',
  'DA_Memory_Drawings_MCogni': 'Cognitive Cards',
  'DA_Memory_Drawings_MSurvival': 'Survival Diagrams',
  'DA_Memory_Drawings_Music': 'Music Notes',
  'DA_Memory_Drawings_Plans': 'Plans',
  'DA_Memory_Guitar': 'Guitar',
  'DA_Memory_Mirror': 'Mirror',
  'DA_Memory_MysteryBox': 'Mystery Box',
  'DA_Memory_SmallTree2': 'Small Tree',
  'DA_Memory_Stopwatch': 'Stopwatch',
  'DA_Memory_Toy': 'Teddy Bear',
};

class Reader {
  view: DataView;
  bytes: Uint8Array;
  offset: number;

  constructor(bytes: Uint8Array, offset = 0) {
    this.bytes = bytes;
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    this.offset = offset;
  }

  ensure(size: number) {
    if (this.offset + size > this.bytes.length) throw new Error('Read is out of range');
  }
  uint8() { this.ensure(1); return this.view.getUint8(this.offset++); }
  int32() { this.ensure(4); const value = this.view.getInt32(this.offset, true); this.offset += 4; return value; }
  uint32() { this.ensure(4); const value = this.view.getUint32(this.offset, true); this.offset += 4; return value; }
  float() { this.ensure(4); const value = this.view.getFloat32(this.offset, true); this.offset += 4; return value; }
  double() { this.ensure(8); const value = this.view.getFloat64(this.offset, true); this.offset += 8; return value; }
  skip(size: number) { this.ensure(size); this.offset += size; }
  take(size: number) { this.ensure(size); const value = this.bytes.subarray(this.offset, this.offset + size); this.offset += size; return value; }
  string() {
    const length = this.int32();
    if (length === 0) return '';
    if (length > 0) return textDecoder.decode(this.take(length - 1)) + (this.skip(1), '');
    const chars = -length;
    const data = this.take(chars * 2 - 2);
    this.skip(2);
    let value = '';
    for (let index = 0; index < data.length; index += 2) value += String.fromCharCode(data[index] | (data[index + 1] << 8));
    return value;
  }
}

function readTypeName(reader: Reader, depth = 0): TypeName {
  if (depth > 12) throw new Error('Type is nested too deeply');
  const name = reader.string();
  const count = reader.int32();
  if (count < 0 || count > 12) throw new Error('Invalid type');
  return { name, parameters: Array.from({ length: count }, () => readTypeName(reader, depth + 1)) };
}

function isTaggedStruct(type: TypeName) {
  const structName = type.parameters[0]?.name ?? '';
  const raw = new Set(['Vector', 'Vector2D', 'Vector4', 'Quat', 'Rotator', 'Transform', 'Guid', 'LinearColor', 'Color', 'DateTime', 'Timespan', 'IntPoint', 'IntVector', 'Box', 'Box2D', 'SoftObjectPath', 'GameplayTag']);
  return !raw.has(structName);
}

function readValue(reader: Reader, type: TypeName, end: number, depth = 0): unknown {
  if (depth > 80) throw new Error('Structure is nested too deeply');
  switch (type.name) {
    case 'IntProperty': return reader.int32();
    case 'UInt32Property': return reader.uint32();
    case 'FloatProperty': return reader.float();
    case 'DoubleProperty': return reader.double();
    case 'TextProperty': {
      reader.uint32();
      const history = reader.uint8();
      if (history === 0xff) {
        reader.int32();
        return reader.string();
      }
      reader.skip(end - reader.offset);
      return '';
    }
    case 'StrProperty':
    case 'NameProperty':
    case 'EnumProperty':
    case 'ObjectProperty':
    case 'SoftObjectProperty':
    case 'ClassProperty': return reader.string();
    case 'StructProperty': return isTaggedStruct(type) ? readPropertyList(reader, end, depth + 1) : (reader.skip(end - reader.offset), null);
    case 'ArrayProperty': {
      const count = reader.int32();
      const inner = type.parameters[0];
      if (!inner || count < 0 || count > 100000) throw new Error('Invalid array');
      if (inner.name === 'ByteProperty') { reader.skip(end - reader.offset); return null; }
      return Array.from({ length: count }, () => readValue(reader, inner, end, depth + 1));
    }
    case 'MapProperty': {
      reader.int32();
      const count = reader.int32();
      const [key, value] = type.parameters;
      if (!key || !value || count < 0 || count > 100000) throw new Error('Invalid map');
      for (let index = 0; index < count; index += 1) {
        readValue(reader, key, end, depth + 1);
        readValue(reader, value, end, depth + 1);
      }
      return null;
    }
    default: reader.skip(end - reader.offset); return null;
  }
}

function readPropertyList(reader: Reader, end: number, depth = 0): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  while (reader.offset < end) {
    const name = reader.string();
    if (name === 'None') break;
    const type = readTypeName(reader);
    const size = reader.uint32();
    const boolValue = type.name === 'BoolProperty' ? reader.uint8() !== 0 : null;
    if (reader.uint8()) reader.skip(16);
    const valueEnd = reader.offset + size;
    result[name] = type.name === 'BoolProperty' ? boolValue : readValue(reader, type, valueEnd, depth + 1);
    reader.offset = valueEnd;
  }
  return result;
}

function findAll(bytes: Uint8Array, pattern: Uint8Array, start = 0, end = bytes.length) {
  const found: number[] = [];
  outer: for (let offset = start; offset <= end - pattern.length; offset += 1) {
    if (bytes[offset] !== pattern[0]) continue;
    for (let index = 1; index < pattern.length; index += 1) if (bytes[offset + index] !== pattern[index]) continue outer;
    found.push(offset);
    offset += pattern.length - 1;
  }
  return found;
}

function fStringPattern(value: string) {
  const encoded = textEncoder.encode(`${value}\0`);
  const result = new Uint8Array(4 + encoded.length);
  new DataView(result.buffer).setInt32(0, encoded.length, true);
  result.set(encoded, 4);
  return result;
}

function readPropertyAt(bytes: Uint8Array, offset: number) {
  const reader = new Reader(bytes, offset);
  const name = reader.string();
  const type = readTypeName(reader);
  const size = reader.uint32();
  const boolValue = type.name === 'BoolProperty' ? reader.uint8() !== 0 : null;
  if (reader.uint8()) reader.skip(16);
  const end = reader.offset + size;
  return { name, value: type.name === 'BoolProperty' ? boolValue : readValue(reader, type, end), end };
}

async function decompressSave(compressed: Uint8Array, progress?: (message: string) => void) {
  const chunks: Uint8Array[] = [];
  let offset = 0;
  let blockCount = 0;
  let total = 0;
  while (offset < compressed.length) {
    if (offset + HEADER_SIZE > compressed.length || MAGIC.some((byte, index) => compressed[offset + index] !== byte)) throw new Error(`Unrecognized block at 0x${offset.toString(16)}`);
    const compressedSize = compressed[offset + 17] | (compressed[offset + 18] << 8) | (compressed[offset + 19] << 16);
    const uncompressedSize = compressed[offset + 25] | (compressed[offset + 26] << 8) | (compressed[offset + 27] << 16);
    const payload = compressed.slice(offset + HEADER_SIZE, offset + HEADER_SIZE + compressedSize);
    const stream = new Blob([payload]).stream().pipeThrough(new DecompressionStream('deflate'));
    const chunk = new Uint8Array(await new Response(stream).arrayBuffer());
    if (chunk.length !== uncompressedSize) throw new Error('A decompressed block has an unexpected size');
    chunks.push(chunk);
    total += chunk.length;
    offset += HEADER_SIZE + compressedSize;
    blockCount += 1;
    if (blockCount % 50 === 0) progress?.(`Decompressing… ${blockCount} blocks`);
  }
  const raw = new Uint8Array(total);
  let write = 0;
  for (const chunk of chunks) { raw.set(chunk, write); write += chunk.length; }
  return { raw, blockCount };
}

function scanItems(bytes: Uint8Array, start: number, end: number): ImportedItem[] {
  const marker = fStringPattern('AssetName');
  const instances: { assetName: string; quantity: number }[] = [];
  for (const offset of findAll(bytes, marker, start, end)) {
    try {
      const reader = new Reader(bytes, offset);
      const value = readPropertyList(reader, Math.min(end, offset + 1024 * 1024));
      const assetName = value.AssetName;
      const data = value.Data as Record<string, unknown> | undefined;
      if (typeof assetName === 'string' && /^DA_(?:Food|Memory)_/.test(assetName)) {
        instances.push({ assetName, quantity: typeof data?.ItemCount === 'number' ? data.ItemCount : 1 });
      }
    } catch {
      // Other AssetName fields are not serialized inventory records.
    }
  }

  if (instances.length % 2 === 0) {
    const half = instances.length / 2;
    const mirrored = instances.slice(0, half).every((item, index) => item.assetName === instances[index + half].assetName && item.quantity === instances[index + half].quantity);
    if (mirrored) instances.splice(half);
  }

  const aggregate = new Map<string, number>();
  for (const item of instances) aggregate.set(item.assetName, (aggregate.get(item.assetName) ?? 0) + item.quantity);
  return [...aggregate].map(([assetName, quantity]) => ({ assetName, mappedName: ASSET_ALIASES[assetName] ?? null, quantity }));
}

function findActorRecords(raw: Uint8Array) {
  const classMarker = fStringPattern('ActorClass');
  const nameMarker = fStringPattern('ActorName');
  const records: { offset: number; classPath: string; actorName: string; end: number }[] = [];
  for (const offset of findAll(raw, classMarker)) {
    try {
      const property = readPropertyAt(raw, offset);
      if (typeof property.value !== 'string' || !property.value.startsWith('/Game/Blueprints/')) continue;
      const nameOffset = findAll(raw, nameMarker, property.end, Math.min(raw.length, property.end + 768))[0];
      const name = nameOffset === undefined ? null : readPropertyAt(raw, nameOffset).value;
      records.push({ offset, classPath: property.value, actorName: typeof name === 'string' ? name : `actor-${records.length + 1}`, end: raw.length });
    } catch {
      // A property with the same name but a different format.
    }
  }
  records.sort((a, b) => a.offset - b.offset);
  records.forEach((record, index) => { record.end = records[index + 1]?.offset ?? raw.length; });
  return records;
}

function containsText(raw: Uint8Array, value: string) {
  if (findAll(raw, textEncoder.encode(value)).length) return true;
  const wide = new Uint8Array(value.length * 2);
  for (let index = 0; index < value.length; index += 1) wide[index * 2] = value.charCodeAt(index);
  return findAll(raw, wide).length > 0;
}

function findContainerLabel(raw: Uint8Array, start: number, end: number) {
  for (const offset of findAll(raw, fStringPattern('Name'), start, end)) {
    try {
      const property = readPropertyAt(raw, offset);
      if (property.name === 'Name' && typeof property.value === 'string' && property.value.trim()) return property.value.trim();
    } catch {
      // Not every Name field is serialized as FText.
    }
  }
  return null;
}

export async function importSave(file: File, progress?: (message: string) => void): Promise<SaveImportResult> {
  progress?.('Reading save…');
  const compressed = new Uint8Array(await file.arrayBuffer());
  const { raw, blockCount } = await decompressSave(compressed, progress);
  progress?.('Locating the character and player chests…');
  const actors = findActorRecords(raw);
  const relevant = actors.filter(({ classPath }) => classPath.includes('BP_FirstPersonCharacter_New.BP_FirstPersonCharacter_New_C') || /BP_Inventory_PlayerBox(?:_Small)?\.BP_Inventory_PlayerBox(?:_Small)?_C$/.test(classPath));
  const sources: InventorySource[] = [];
  for (const actor of relevant) {
    const kind = actor.classPath.includes('BP_FirstPersonCharacter_New') ? 'backpack' : 'player-box';
    const items = scanItems(raw, actor.offset, actor.end);
    if (!items.length && kind === 'player-box') continue;
    const containerLabel = kind === 'player-box' ? findContainerLabel(raw, actor.offset, actor.end) : null;
    sources.push({
      id: `${kind}:${actor.actorName}`,
      kind,
      label: kind === 'backpack' ? 'Character backpack' : `Player chest · ${containerLabel ?? actor.actorName}`,
      items,
    });
  }
  if (!sources.some((source) => source.kind === 'backpack')) throw new Error('The character backpack could not be identified in this save version.');
  progress?.('Inventory verified');
  return { sources, customLabelFound: containsText(raw, 'PruebaItems001'), rawBytes: raw.length, blockCount };
}
