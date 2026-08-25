import { invoke } from '@tauri-apps/api/core';
import type {
  BootstrapState,
  CatalogueBundle,
  CsvDocument,
  HumanAssessment,
  InventoryReport,
  RecipeResult,
  Settings,
  SyncApplyResult,
  SyncProposal,
} from './types';

export const api = {
  bootstrap: () => invoke<BootstrapState>('bootstrap'),
  saveConfiguration: (configuration: Settings) => invoke<Settings>('save_configuration', { configuration }),
  refreshInventory: (configuration: Settings) => invoke<InventoryReport>('refresh_inventory', { configuration }),
  reloadCatalogues: () => invoke<CatalogueBundle>('reload_catalogues'),
  loadAssetMappings: () => invoke<Record<string, string>>('load_asset_mappings'),
  saveAssetMappings: (mappings: Record<string, string>) => invoke<Record<string, string>>('save_asset_mappings', { mappings }),
  importAssetMappings: (sourcePath: string) => invoke<Record<string, string>>('import_asset_mappings', { sourcePath }),
  exportAssetMappings: (targetPath: string) => invoke<void>('export_asset_mappings', { targetPath }),
  saveCatalogue: (document: CsvDocument) => invoke<CsvDocument>('save_catalogue', { document }),
  importCatalogue: (kind: string, sourcePath: string) => invoke<CsvDocument>('import_catalogue', { kind, sourcePath }),
  exportCatalogue: (kind: string, targetPath: string) => invoke<void>('export_catalogue', { kind, targetPath }),
  scanGameData: (gamePath?: string | null) => invoke<SyncProposal>('scan_game_data', { gamePath: gamePath ?? null }),
  applyGameDataSync: (proposal: SyncProposal, selectedIds: string[]) =>
    invoke<SyncApplyResult>('apply_game_data_sync', { proposal, selectedIds }),
  assessHumans: (inventory: Record<string, number>) => invoke<HumanAssessment[]>('assess_humans', { inventory }),
  calculateRecipe: (inventory: Record<string, number>, profession: string, objective: string) =>
    invoke<RecipeResult>('calculate_recipe', { inventory, profession, objective }),
};
