export type Settings = {
  savePath: string | null;
  chestNames: string[];
};

export type CsvDocument = {
  kind: 'food' | 'memories' | 'humans';
  fileName: string;
  headers: string[];
  rows: string[][];
};

export type CatalogueBundle = {
  food: CsvDocument;
  memories: CsvDocument;
  humans: CsvDocument;
};

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

export type InventoryReport = {
  savePath: string;
  fileSize: number;
  modifiedUnixMs: number;
  rawBytes: number;
  blockCount: number;
  sources: InventorySource[];
  discoveredChests: string[];
  missingChests: string[];
  inventory: Record<string, number>;
  unresolvedAssets: string[];
  warnings: string[];
};

export type BootstrapState = {
  settings: Settings;
  catalogues: CatalogueBundle;
  assetMappings: Record<string, string>;
  portableRoot: string;
  defaultSaveDirectory: string | null;
};

export type HumanAssessment = {
  category: string;
  profession: string;
  achievable: boolean;
  coveragePercent: number;
  deficits: Record<string, number>;
};

export type RecipePick = {
  itemName: string;
  itemType: 'food' | 'memory';
  quantity: number;
};

export type RecipeResult = {
  profession: string;
  feasible: boolean;
  picks: RecipePick[];
  totals: Record<string, number>;
  requirements: Record<string, number>;
  deficits: Record<string, number>;
  excess: Record<string, number>;
  itemCount: number;
  waste: number;
  matchedProfessions: string[];
};
