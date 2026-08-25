import { useEffect, useMemo, useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';
import {
  ArchiveRestore,
  Box,
  Calculator,
  Check,
  ChevronRight,
  CircleAlert,
  Database,
  Download,
  FileInput,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  PackageOpen,
  Plus,
  RefreshCw,
  Save,
  Search,
  ShieldCheck,
  Trash2,
  Upload,
  UserRound,
  X,
} from 'lucide-react';
import { api } from './api';
import type {
  BootstrapState,
  CatalogueBundle,
  CsvDocument,
  HumanAssessment,
  InventoryReport,
  RecipeResult,
  Settings,
  SyncProposal,
} from './types';

type View = 'inventory' | 'data' | 'calculator';
type CatalogueKind = keyof CatalogueBundle;
type DataKind = CatalogueKind | 'mappings';

const emptySettings: Settings = { savePath: null, chestNames: [], gamePath: null };

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MiB`;
}

function fileName(path: string | null) {
  if (!path) return 'No save selected';
  return path.split(/[\\/]/).at(-1) ?? path;
}

function cloneDocument(document: CsvDocument): CsvDocument {
  return { ...document, headers: [...document.headers], rows: document.rows.map((row) => [...row]) };
}

export default function App() {
  const [view, setView] = useState<View>('inventory');
  const [bootstrap, setBootstrap] = useState<BootstrapState | null>(null);
  const [settings, setSettings] = useState<Settings>(emptySettings);
  const [catalogues, setCatalogues] = useState<CatalogueBundle | null>(null);
  const [assetMappings, setAssetMappings] = useState<Record<string, string>>({});
  const [inventoryReport, setInventoryReport] = useState<InventoryReport | null>(null);
  const [workingInventory, setWorkingInventory] = useState<Record<string, number>>({});
  const [assessments, setAssessments] = useState<HumanAssessment[]>([]);
  const [selectedProfession, setSelectedProfession] = useState('');
  const [recipe, setRecipe] = useState<RecipeResult | null>(null);
  const [objective, setObjective] = useState<'waste' | 'items'>('waste');
  const [dataKind, setDataKind] = useState<DataKind>('food');
  const [draftDocument, setDraftDocument] = useState<CsvDocument | null>(null);
  const [draftMappings, setDraftMappings] = useState<[string, string][]>([]);
  const [dataDirty, setDataDirty] = useState(false);
  const [dataSearch, setDataSearch] = useState('');
  const [pendingMappingAsset, setPendingMappingAsset] = useState<string | null>(null);
  const [newChestName, setNewChestName] = useState('');
  const [humanSearch, setHumanSearch] = useState('');
  const [achievableOnly, setAchievableOnly] = useState(false);
  const [busy, setBusy] = useState('Starting local workspace…');
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [syncProposal, setSyncProposal] = useState<SyncProposal | null>(null);
  const [syncSection, setSyncSection] = useState<DataKind>('food');
  const [selectedSyncIds, setSelectedSyncIds] = useState<string[]>([]);

  useEffect(() => {
    void (async () => {
      try {
        const state = await api.bootstrap();
        setBootstrap(state);
        setSettings(state.settings);
        setCatalogues(state.catalogues);
        setAssetMappings(state.assetMappings);
        setDraftMappings(Object.entries(state.assetMappings));
        setDraftDocument(cloneDocument(state.catalogues.food));
        if (state.settings.savePath) {
          await refreshInventory(state.settings, true);
        }
      } catch (caught) {
        setError(errorMessage(caught));
      } finally {
        setBusy('');
      }
    })();
  }, []);

  useEffect(() => {
    if (!catalogues) return;
    if (dataKind === 'mappings') {
      setDraftDocument(null);
      setDraftMappings(Object.entries(assetMappings));
    } else {
      setDraftDocument(cloneDocument(catalogues[dataKind]));
    }
    setDataDirty(false);
    setDataSearch('');
  }, [dataKind, catalogues, assetMappings]);

  useEffect(() => {
    if (dataKind !== 'mappings' || !pendingMappingAsset) return;
    const alreadyMapped = Object.prototype.hasOwnProperty.call(assetMappings, pendingMappingAsset);
    setDraftMappings((current) => current.some(([asset]) => asset === pendingMappingAsset) ? current : [...current, [pendingMappingAsset, '']]);
    setDataSearch(pendingMappingAsset);
    if (!alreadyMapped) setDataDirty(true);
    setPendingMappingAsset(null);
  }, [dataKind, pendingMappingAsset, assetMappings]);

  useEffect(() => {
    if (!catalogues) return;
    void (async () => {
      try {
        const result = await api.assessHumans(workingInventory);
        setAssessments(result);
        if (!selectedProfession || !result.some((human) => human.profession === selectedProfession)) {
          setSelectedProfession(result.find((human) => human.achievable)?.profession ?? result[0]?.profession ?? '');
        }
      } catch (caught) {
        setError(errorMessage(caught));
      }
    })();
  }, [catalogues, workingInventory]);

  async function persistSettings(next: Settings) {
    const saved = await api.saveConfiguration(next);
    setSettings(saved);
    return saved;
  }

  async function chooseSave() {
    const selected = await open({
      multiple: false,
      directory: false,
      title: 'Select The Last Caretaker save',
      defaultPath: bootstrap?.defaultSaveDirectory ?? undefined,
      filters: [{ name: 'The Last Caretaker save', extensions: ['sav'] }],
    });
    if (typeof selected !== 'string') return;
    setError('');
    try {
      const next = await persistSettings({ ...settings, savePath: selected });
      await refreshInventory(next);
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }

  async function refreshInventory(configuration = settings, quiet = false) {
    if (!configuration.savePath) return;
    setBusy('Creating a stable read-only snapshot…');
    setError('');
    if (!quiet) setNotice('');
    try {
      const report = await api.refreshInventory(configuration);
      setInventoryReport(report);
      setWorkingInventory({ ...report.inventory });
      setRecipe(null);
      if (!quiet) setNotice(`Inventory refreshed from ${fileName(report.savePath)}.`);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy('');
    }
  }

  async function addChest(name = newChestName) {
    const normalized = name.trim();
    if (!normalized || settings.chestNames.some((existing) => existing.toLowerCase() === normalized.toLowerCase())) return;
    try {
      await persistSettings({ ...settings, chestNames: [...settings.chestNames, normalized] });
      setNewChestName('');
      setNotice(`Chest “${normalized}” added. Refresh the save to include it.`);
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }

  async function removeChest(name: string) {
    try {
      await persistSettings({ ...settings, chestNames: settings.chestNames.filter((existing) => existing !== name) });
      setNotice(`Chest “${name}” removed. Refresh the save to update inventory.`);
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }

  function updateCell(rowIndex: number, columnIndex: number, value: string) {
    setDraftDocument((current) => {
      if (!current) return current;
      const rows = current.rows.map((row, index) => index === rowIndex ? row.map((cell, column) => column === columnIndex ? value : cell) : row);
      return { ...current, rows };
    });
    setDataDirty(true);
  }

  function updateMapping(rowIndex: number, columnIndex: 0 | 1, value: string) {
    setDraftMappings((current) => current.map((row, index) => index === rowIndex ? row.map((cell, column) => column === columnIndex ? value : cell) as [string, string] : row));
    setDataDirty(true);
  }

  function openAssetMapping(assetName: string) {
    setPendingMappingAsset(assetName);
    setView('data');
    setDataKind('mappings');
  }

  function addCatalogueRow() {
    setDraftDocument((current) => current ? { ...current, rows: [...current.rows, current.headers.map(() => '')] } : current);
    setDataSearch('');
    setDataDirty(true);
  }

  function removeCatalogueRow(rowIndex: number) {
    setDraftDocument((current) => current ? { ...current, rows: current.rows.filter((_, index) => index !== rowIndex) } : current);
    setDataDirty(true);
  }

  async function saveDataChanges() {
    if (!catalogues) return;
    setBusy(`Saving ${dataKind === 'mappings' ? 'asset-mappings.json' : draftDocument?.fileName ?? 'catalogue'}…`);
    setError('');
    try {
      if (dataKind === 'mappings') {
        const normalized = draftMappings.map(([asset, target]) => [asset.trim(), target.trim()] as const);
        if (normalized.some(([asset, target]) => !asset || !target)) throw new Error('Every asset mapping needs both an asset name and a catalogue item.');
        if (new Set(normalized.map(([asset]) => asset)).size !== normalized.length) throw new Error('Asset mapping names must be unique.');
        const saved = await api.saveAssetMappings(Object.fromEntries(normalized));
        setAssetMappings(saved);
        setDraftMappings(Object.entries(saved));
        setDataDirty(false);
        setNotice('asset-mappings.json saved. A backup was created before replacement.');
        return;
      }
      if (!draftDocument) return;
      const saved = await api.saveCatalogue(draftDocument);
      setCatalogues({ ...catalogues, [dataKind]: saved });
      setDraftDocument(cloneDocument(saved));
      setDataDirty(false);
      setRecipe(null);
      setNotice(`${saved.fileName} saved. A backup was created before replacement.`);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy('');
    }
  }

  async function reloadData() {
    setBusy('Reloading game data files…');
    try {
      const [loaded, mappings] = await Promise.all([api.reloadCatalogues(), api.loadAssetMappings()]);
      setCatalogues(loaded);
      setAssetMappings(mappings);
      if (dataKind === 'mappings') setDraftMappings(Object.entries(mappings));
      else setDraftDocument(cloneDocument(loaded[dataKind]));
      setDataDirty(false);
      setNotice('Game data files reloaded from the portable data folder.');
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy('');
    }
  }

  async function importData() {
    const isMappings = dataKind === 'mappings';
    const selected = await open({ multiple: false, directory: false, title: `Import ${isMappings ? 'asset mappings' : dataKind}`, filters: [{ name: isMappings ? 'JSON file' : 'CSV file', extensions: [isMappings ? 'json' : 'csv'] }] });
    if (typeof selected !== 'string' || !catalogues) return;
    setBusy(`Validating imported ${dataKind} data…`);
    try {
      if (isMappings) {
        const mappings = await api.importAssetMappings(selected);
        setAssetMappings(mappings);
        setDraftMappings(Object.entries(mappings));
        setDataDirty(false);
        setNotice('asset-mappings.json imported and validated.');
        return;
      }
      const document = await api.importCatalogue(dataKind, selected);
      setCatalogues({ ...catalogues, [dataKind]: document });
      setDraftDocument(cloneDocument(document));
      setDataDirty(false);
      setNotice(`${document.fileName} imported and validated.`);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy('');
    }
  }

  async function exportData() {
    const isMappings = dataKind === 'mappings';
    const target = await save({ title: `Export ${isMappings ? 'asset mappings' : dataKind}`, defaultPath: isMappings ? 'asset-mappings.json' : draftDocument?.fileName ?? `${dataKind}.csv`, filters: [{ name: isMappings ? 'JSON file' : 'CSV file', extensions: [isMappings ? 'json' : 'csv'] }] });
    if (!target) return;
    try {
      if (isMappings) await api.exportAssetMappings(target);
      else await api.exportCatalogue(dataKind, target);
      setNotice(`Exported ${isMappings ? 'asset-mappings.json' : draftDocument?.fileName ?? 'catalogue'} successfully.`);
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }

  async function syncFromGame(chooseFolder = false) {
    if (dataDirty && !confirm('Discard unsaved data changes before scanning the installed game?')) return;
    let gamePath = settings.gamePath ?? bootstrap?.defaultGameDirectory ?? null;
    if (chooseFolder || !gamePath) {
      const selected = await open({
        multiple: false,
        directory: true,
        title: 'Select The Last Caretaker game folder',
        defaultPath: gamePath ?? undefined,
      });
      if (typeof selected !== 'string') return;
      gamePath = selected;
    }
    setBusy('Reading installed game catalogues…');
    setError('');
    setNotice('');
    try {
      const proposal = await api.scanGameData(gamePath);
      setSettings((current) => ({ ...current, gamePath: proposal.source.gamePath }));
      setSyncProposal(proposal);
      setSyncSection(dataKind);
      setSelectedSyncIds(proposal.sections.flatMap((section) => section.changes.filter((change) => change.selectedByDefault).map((change) => change.id)));
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy('');
    }
  }

  async function applyGameSync() {
    if (!syncProposal || selectedSyncIds.length === 0) return;
    setBusy(`Applying ${selectedSyncIds.length} reviewed game-data changes…`);
    setError('');
    try {
      const result = await api.applyGameDataSync(syncProposal, selectedSyncIds);
      setCatalogues(result.catalogues);
      setAssetMappings(result.assetMappings);
      if (dataKind === 'mappings') setDraftMappings(Object.entries(result.assetMappings));
      else setDraftDocument(cloneDocument(result.catalogues[dataKind]));
      setDataDirty(false);
      setSyncProposal(null);
      setSelectedSyncIds([]);
      setRecipe(null);
      setNotice(`${result.appliedCount} reviewed changes applied. Backups were created before replacing local data files.`);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy('');
    }
  }

  async function calculate() {
    if (!selectedProfession) return;
    setBusy(`Optimizing ${selectedProfession}…`);
    setError('');
    try {
      setRecipe(await api.calculateRecipe(workingInventory, selectedProfession, objective));
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy('');
    }
  }

  const filteredAssessments = useMemo(() => assessments.filter((human) => {
    const matchesSearch = `${human.category} ${human.profession}`.toLowerCase().includes(humanSearch.toLowerCase());
    return matchesSearch && (!achievableOnly || human.achievable);
  }), [assessments, humanSearch, achievableOnly]);

  const normalizedDataSearch = dataSearch.trim().toLowerCase();
  const visibleMappingRows = useMemo(() => draftMappings
    .map((row, index) => ({ row, index }))
    .filter(({ row }) => !normalizedDataSearch || row.some((cell) => cell.toLowerCase().includes(normalizedDataSearch))),
  [draftMappings, normalizedDataSearch]);
  const visibleCatalogueRows = useMemo(() => (draftDocument?.rows ?? [])
    .map((row, index) => ({ row, index }))
    .filter(({ row }) => !normalizedDataSearch || row.some((cell) => cell.toLowerCase().includes(normalizedDataSearch))),
  [draftDocument, normalizedDataSearch]);

  const unresolvedDiagnostics = useMemo(() => (inventoryReport?.unresolvedAssets ?? []).map((assetName) => {
    const sources = (inventoryReport?.sources ?? []).map((source) => {
      const matchingItems = source.items.filter((item) => item.assetName === assetName);
      return {
        id: source.id,
        label: source.label,
        kind: source.kind,
        quantity: matchingItems.reduce((sum, item) => sum + item.quantity, 0),
        entries: matchingItems.length,
      };
    }).filter((source) => source.quantity > 0);
    const itemType = assetName.startsWith('DA_Food_') ? 'Food' : assetName.startsWith('DA_Memory_') ? 'Memory' : 'Unknown type';
    const nameHint = assetName.replace(/^DA_(?:Food|Memory)_/, '').replace(/[_-]+/g, ' ').trim();
    return {
      assetName,
      itemType,
      nameHint,
      sources,
      quantity: sources.reduce((sum, source) => sum + source.quantity, 0),
      entries: sources.reduce((sum, source) => sum + source.entries, 0),
    };
  }), [inventoryReport]);

  const selectedAssessment = assessments.find((human) => human.profession === selectedProfession);
  const inventoryUnits = Object.values(workingInventory).reduce((sum, quantity) => sum + quantity, 0);
  const visibleSyncChanges = syncProposal?.sections.find((section) => section.kind === syncSection)?.changes ?? [];

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark"><UserRound size={20} /></div>
          <div><strong>CAREKEEPER</strong><span>Human Growth Utility</span></div>
        </div>
        <nav>
          <button className={view === 'inventory' ? 'active' : ''} onClick={() => setView('inventory')}><HardDrive size={18} /> Save & inventory</button>
          <button className={view === 'data' ? 'active' : ''} onClick={() => setView('data')}><Database size={18} /> Game data</button>
          <button className={view === 'calculator' ? 'active' : ''} onClick={() => setView('calculator')}><Calculator size={18} /> Human calculator</button>
        </nav>
        <div className="sidebar-status">
          <span className={inventoryReport ? 'status-dot online' : 'status-dot'} />
          <div><strong>{inventoryReport ? 'Inventory loaded' : 'Awaiting save'}</strong><span>{inventoryUnits} usable units</span></div>
        </div>
        <div className="portable-path" title={bootstrap?.portableRoot}><ArchiveRestore size={15} /><span>Portable workspace<br />{bootstrap?.portableRoot ?? 'Starting…'}</span></div>
      </aside>

      <main>
        <header className="topbar">
          <div>
            <span className="eyebrow">LOCAL · READ ONLY SAVE ACCESS</span>
            <h1>{view === 'inventory' ? 'Save & inventory' : view === 'data' ? 'Game data' : 'Human calculator'}</h1>
          </div>
          <div className="topbar-actions">
            <div className="save-chip"><ShieldCheck size={17} /><div><strong>{fileName(settings.savePath)}</strong><span>{inventoryReport ? new Date(Number(inventoryReport.modifiedUnixMs)).toLocaleString() : 'Not read yet'}</span></div></div>
            <button className="button secondary" onClick={chooseSave}><FolderOpen size={17} /> Choose save</button>
            <button className="button primary" disabled={!settings.savePath || !!busy} onClick={() => refreshInventory()}><RefreshCw size={17} /> Refresh</button>
          </div>
        </header>

        {(busy || error || notice) && (
          <div className={`message-bar ${error ? 'error' : notice ? 'success' : ''}`}>
            {busy ? <LoaderCircle className="spin" size={17} /> : error ? <CircleAlert size={17} /> : <Check size={17} />}
            <span>{busy || error || notice}</span>
            {(error || notice) && <button onClick={() => { setError(''); setNotice(''); }}>Dismiss</button>}
          </div>
        )}

        {view === 'inventory' && (
          <div className="page-grid inventory-page">
            <section className="panel save-panel">
              <div className="panel-heading">
                <div><span className="section-index">01</span><h2>Save source</h2><p>The file is opened for reading only. A changed snapshot is discarded and retried.</p></div>
                <ShieldCheck className="accent-icon" size={26} />
              </div>
              <div className="file-source">
                <div className="file-icon"><FileInput size={24} /></div>
                <div className="file-details"><strong>{fileName(settings.savePath)}</strong><span>{settings.savePath ?? 'Choose the save used by your current playthrough.'}</span></div>
                <button className="button secondary" onClick={chooseSave}>Browse</button>
              </div>
              <p className="default-save-hint"><FolderOpen size={15} /> Default game folder: <code>%LOCALAPPDATA%\Voyage\Saved\SaveGames</code></p>
              {inventoryReport && (
                <div className="metrics-row">
                  <div><span>Compressed</span><strong>{formatBytes(inventoryReport.fileSize)}</strong></div>
                  <div><span>Validated data</span><strong>{formatBytes(inventoryReport.rawBytes)}</strong></div>
                  <div><span>Blocks</span><strong>{inventoryReport.blockCount}</strong></div>
                  <div><span>Sources used</span><strong>{inventoryReport.sources.length}</strong></div>
                </div>
              )}
            </section>

            <section className="panel chests-panel">
              <div className="panel-heading compact">
                <div><span className="section-index">02</span><h2>Player chests</h2><p>Add exact custom chest names. Environmental containers are always excluded.</p></div>
              </div>
              <div className="inline-form">
                <input value={newChestName} onChange={(event) => setNewChestName(event.target.value)} onKeyDown={(event) => event.key === 'Enter' && void addChest()} placeholder="Custom chest name" />
                <button className="button primary" onClick={() => addChest()}><Plus size={17} /> Add</button>
              </div>
              <div className="tag-list">
                {settings.chestNames.length === 0 && <p className="empty-note">Only the player backpack will be used.</p>}
                {settings.chestNames.map((name) => (
                  <div className={`tag ${inventoryReport?.missingChests.includes(name) ? 'missing' : ''}`} key={name}>
                    <Box size={15} /><span>{name}</span>{inventoryReport?.missingChests.includes(name) && <em>not found</em>}
                    <button aria-label={`Remove ${name}`} onClick={() => removeChest(name)}><Trash2 size={14} /></button>
                  </div>
                ))}
              </div>
              {!!inventoryReport?.discoveredChests.length && (
                <div className="discovered">
                  <span>Discovered player boxes</span>
                  {inventoryReport.discoveredChests.filter((name) => !settings.chestNames.some((configured) => configured.toLowerCase() === name.toLowerCase())).map((name) => (
                    <button key={name} onClick={() => addChest(name)}><Plus size={13} /> {name}</button>
                  ))}
                </div>
              )}
            </section>

            <section className="panel inventory-panel full-span">
              <div className="panel-heading compact">
                <div><span className="section-index">03</span><h2>Usable growth inventory</h2><p>Quantities can be adjusted for planning. Refreshing the save restores imported values.</p></div>
                <div className="inventory-total"><strong>{inventoryUnits}</strong><span>total units</span></div>
              </div>
              {!!unresolvedDiagnostics.length && (
                <div className="unresolved-box">
                  <div className="unresolved-heading">
                    <CircleAlert size={18} />
                    <div><strong>{unresolvedDiagnostics.length} unresolved game {unresolvedDiagnostics.length === 1 ? 'asset' : 'assets'} found</strong><span>They are present in the selected inventory sources but excluded from the calculator until assigned to verified catalogue items.</span></div>
                    <button className="button ghost" onClick={() => { setView('data'); setDataKind('mappings'); }}>View all mappings</button>
                  </div>
                  <div className="unresolved-list">
                    {unresolvedDiagnostics.map((diagnostic) => (
                      <article className="unresolved-item" key={diagnostic.assetName}>
                        <div className="unresolved-asset"><code>{diagnostic.assetName}</code><span>{diagnostic.itemType}</span></div>
                        <div className="unresolved-metrics">
                          <div><strong>{diagnostic.quantity}</strong><span>items found</span></div>
                          <div><strong>{diagnostic.sources.length}</strong><span>sources</span></div>
                          <div><strong>{diagnostic.entries}</strong><span>save entries</span></div>
                        </div>
                        <div className="unresolved-hint"><span>Name hint</span><strong>{diagnostic.nameHint || 'No readable hint'}</strong></div>
                        <div className="unresolved-sources">
                          {diagnostic.sources.map((source) => <span key={source.id}>{source.kind === 'backpack' ? 'Backpack' : source.label}: <strong>{source.quantity}</strong></span>)}
                        </div>
                        <button className="button ghost unresolved-map-button" onClick={() => openAssetMapping(diagnostic.assetName)}><Plus size={14} /> Create mapping</button>
                      </article>
                    ))}
                  </div>
                </div>
              )}
              {Object.keys(workingInventory).length ? (
                <div className="inventory-table">
                  {Object.entries(workingInventory).sort(([left], [right]) => left.localeCompare(right)).map(([name, quantity]) => (
                    <label key={name}><span>{name}</span><input type="number" min="0" value={quantity} onChange={(event) => { setWorkingInventory((current) => ({ ...current, [name]: Math.max(0, Number(event.target.value) || 0) })); setRecipe(null); }} /></label>
                  ))}
                </div>
              ) : <div className="empty-state"><PackageOpen size={34} /><strong>No inventory loaded</strong><span>Select and refresh a save to import the player backpack.</span></div>}
              {!!inventoryReport?.sources.length && <div className="source-strip">{inventoryReport.sources.map((source) => <div key={source.id}><span>{source.kind === 'backpack' ? 'BACKPACK' : 'PLAYER CHEST'}</span><strong>{source.label}</strong><em>{source.items.reduce((sum, item) => sum + item.quantity, 0)} units</em></div>)}</div>}
            </section>
          </div>
        )}

        {view === 'data' && catalogues && (dataKind === 'mappings' || draftDocument) && (
          <section className="panel data-panel">
            <div className="data-toolbar">
              <div className="segmented">
                {(['food', 'memories', 'humans', 'mappings'] as DataKind[]).map((kind) => <button className={dataKind === kind ? 'active' : ''} key={kind} onClick={() => { if (!dataDirty || confirm('Discard unsaved data changes?')) setDataKind(kind); }}>{kind}</button>)}
              </div>
              <div className="toolbar-actions">
                <button className="button ghost sync-button" disabled={!!busy} onClick={() => syncFromGame()}><Database size={16} /> Sync from game</button>
                <button className="button ghost" onClick={reloadData}><RefreshCw size={16} /> Reload</button>
                <button className="button ghost" onClick={importData}><Upload size={16} /> Import {dataKind === 'mappings' ? 'JSON' : 'CSV'}</button>
                <button className="button ghost" onClick={exportData}><Download size={16} /> Export {dataKind === 'mappings' ? 'JSON' : 'CSV'}</button>
                <button className="button primary" disabled={!dataDirty || !!busy} onClick={saveDataChanges}><Save size={16} /> Save changes</button>
              </div>
            </div>
            <div className="data-intro"><div><span className="section-index">DATA</span><h2>{dataKind === 'mappings' ? 'asset-mappings.json' : draftDocument?.fileName}</h2><p>{dataKind === 'mappings' ? 'Each asset must map to an existing item of the correct type. Ambiguous aliases should remain absent until verified.' : 'Edit, add or remove entries here. Every save validates columns, numbers and duplicate names, then creates a backup.'}</p></div><span className={`dirty-badge ${dataDirty ? 'dirty' : ''}`}>{dataDirty ? 'Unsaved edits' : 'File synchronized'}</span></div>
            <div className="data-filter-row">
              <label className="data-search"><Search size={15} /><input value={dataSearch} onChange={(event) => setDataSearch(event.target.value)} placeholder={`Search ${dataKind} entries`} /></label>
              <span>{dataKind === 'mappings' ? visibleMappingRows.length : visibleCatalogueRows.length} / {dataKind === 'mappings' ? draftMappings.length : draftDocument?.rows.length ?? 0} entries</span>
            </div>
            <div className="table-scroll">
              {dataKind === 'mappings' ? (
                <table className="data-table mapping-table">
                  <thead><tr><th>Game asset</th><th>Catalogue item</th><th /></tr></thead>
                  <tbody>{visibleMappingRows.map(({ row: [asset, target], index: rowIndex }) => {
                    const options = (asset.startsWith('DA_Food_') ? catalogues.food : catalogues.memories).rows.map((row) => row[0]);
                    return <tr key={rowIndex}><td><input aria-label={`Game asset row ${rowIndex + 1}`} value={asset} onChange={(event) => updateMapping(rowIndex, 0, event.target.value)} placeholder="DA_Memory_…" /></td><td><select aria-label={`Catalogue item row ${rowIndex + 1}`} value={target} onChange={(event) => updateMapping(rowIndex, 1, event.target.value)}><option value="">Select a verified item…</option>{options.map((name) => <option key={name}>{name}</option>)}</select></td><td><button className="icon-button" aria-label={`Remove mapping ${asset}`} onClick={() => { setDraftMappings((current) => current.filter((_, index) => index !== rowIndex)); setDataDirty(true); }}><Trash2 size={15} /></button></td></tr>;
                  })}{visibleMappingRows.length === 0 && <tr><td className="no-data-results" colSpan={3}>No mappings match “{dataSearch}”.</td></tr>}</tbody>
                </table>
              ) : draftDocument && (
                <table className="data-table catalogue-table">
                  <thead><tr>{draftDocument.headers.map((header) => <th key={header}>{header}</th>)}<th /></tr></thead>
                  <tbody>{visibleCatalogueRows.map(({ row, index: rowIndex }) => <tr key={rowIndex}>{row.map((cell, columnIndex) => <td key={draftDocument.headers[columnIndex]}><input aria-label={`${draftDocument.headers[columnIndex]} row ${rowIndex + 1}`} value={cell} onChange={(event) => updateCell(rowIndex, columnIndex, event.target.value)} /></td>)}<td><button className="icon-button" aria-label={`Remove ${dataKind} row ${rowIndex + 1}`} onClick={() => removeCatalogueRow(rowIndex)}><Trash2 size={15} /></button></td></tr>)}{visibleCatalogueRows.length === 0 && <tr><td className="no-data-results" colSpan={draftDocument.headers.length + 1}>No {dataKind} entries match “{dataSearch}”.</td></tr>}</tbody>
                </table>
              )}
            </div>
            <button className="button secondary add-data-row" onClick={dataKind === 'mappings' ? () => { setDraftMappings((current) => [...current, ['', '']]); setDataSearch(''); setDataDirty(true); } : addCatalogueRow}><Plus size={16} /> Add {dataKind === 'mappings' ? 'asset mapping' : dataKind === 'memories' ? 'memory' : dataKind === 'humans' ? 'human' : 'food'} entry</button>
          </section>
        )}

        {view === 'calculator' && (
          <div className="calculator-layout">
            <section className="panel human-list-panel">
              <div className="panel-heading compact"><div><span className="section-index">01</span><h2>Profession</h2><p>Availability uses the current backpack, configured chests and manual planning adjustments.</p></div></div>
              <div className="search-field"><Search size={16} /><input value={humanSearch} onChange={(event) => setHumanSearch(event.target.value)} placeholder="Search professions" /></div>
              <label className="check-row"><input type="checkbox" checked={achievableOnly} onChange={(event) => setAchievableOnly(event.target.checked)} /> Show achievable only <span>{assessments.filter((human) => human.achievable).length}/{assessments.length}</span></label>
              <div className="human-list">
                {filteredAssessments.map((human) => (
                  <button className={selectedProfession === human.profession ? 'selected' : ''} key={human.profession} onClick={() => { setSelectedProfession(human.profession); setRecipe(null); }}>
                    <span className={human.achievable ? 'availability yes' : 'availability no'}>{human.achievable ? <Check size={14} /> : `${Math.round(human.coveragePercent)}%`}</span>
                    <div><strong>{human.profession}</strong><span>{human.category}</span></div><ChevronRight size={16} />
                  </button>
                ))}
              </div>
            </section>

            <section className="panel recipe-panel">
              <div className="recipe-hero">
                <div><span className="eyebrow">TARGET PROFESSION</span><h2>{selectedProfession || 'Select a profession'}</h2><p>{selectedAssessment?.achievable ? 'Your current inventory can satisfy every known requirement.' : 'The optimizer will show the best available attempt and remaining deficits.'}</p></div>
                <span className={`hero-status ${selectedAssessment?.achievable ? 'ready' : ''}`}>{selectedAssessment?.achievable ? 'ACHIEVABLE' : 'INCOMPLETE'}</span>
              </div>
              <div className="objective-row"><div><strong>Optimization objective</strong><span>Choose how scarce resources should be protected.</span></div><div className="segmented"><button className={objective === 'waste' ? 'active' : ''} onClick={() => setObjective('waste')}>Lowest excess</button><button className={objective === 'items' ? 'active' : ''} onClick={() => setObjective('items')}>Fewest items</button></div><button className="button primary calculate-button" disabled={!selectedProfession || !!busy} onClick={calculate}><Calculator size={17} /> Calculate recipe</button></div>

              {!recipe ? <div className="empty-state recipe-empty"><Calculator size={38} /><strong>Ready to calculate</strong><span>The recipe never changes the save or consumes inventory automatically.</span></div> : (
                <div className="recipe-results">
                  <div className="result-metrics"><div><span>Status</span><strong className={recipe.feasible ? 'positive' : 'negative'}>{recipe.feasible ? 'Complete' : 'Best attempt'}</strong></div><div><span>Items</span><strong>{recipe.itemCount}</strong></div><div><span>Total excess</span><strong>{recipe.waste}</strong></div><div><span>Profession matches</span><strong>{recipe.matchedProfessions.length}</strong></div></div>
                  <div className="result-columns">
                    <div><h3>Recommended items</h3><div className="pick-list">{recipe.picks.length ? recipe.picks.map((pick) => <div key={pick.itemName}><span className={pick.itemType}>{pick.itemType}</span><strong>{pick.itemName}</strong><em>× {pick.quantity}</em></div>) : <p className="empty-note">No useful item combination was found.</p>}</div></div>
                    <div><h3>Stat fit</h3><div className="stat-list">{Object.entries(recipe.requirements).filter(([, required]) => required > 0).map(([stat, required]) => { const total = recipe.totals[stat] ?? 0; const complete = total >= required; return <div key={stat}><span>{stat}</span><div className="stat-track"><i style={{ width: `${Math.min(100, total / required * 100)}%` }} className={complete ? 'complete' : ''} /></div><strong className={complete ? '' : 'negative'}>{total} / {required}</strong></div>; })}</div></div>
                  </div>
                  <div className="collision-box"><CircleAlert size={18} /><div><strong>Possible profession matches</strong><span>A recipe can satisfy more than one profession. The game's tie-breaking rule is not confirmed, so this list is shown instead of claiming a guaranteed result.</span><div className="match-tags">{recipe.matchedProfessions.map((profession) => <span className={profession === recipe.profession ? 'target' : ''} key={profession}>{profession}</span>)}</div></div></div>
                </div>
              )}
            </section>
          </div>
        )}

        {syncProposal && (
          <div className="sync-overlay" role="dialog" aria-modal="true" aria-label="Review installed game changes">
            <section className="sync-dialog panel">
              <div className="sync-header">
                <div><span className="eyebrow">INSTALLED GAME · REVIEW BEFORE APPLYING</span><h2>Game data synchronization</h2><p title={syncProposal.source.gamePath}>{syncProposal.source.gamePath}</p></div>
                <button className="icon-button sync-close" aria-label="Close synchronization review" onClick={() => setSyncProposal(null)}><X size={20} /></button>
              </div>
              <div className="sync-source-strip">
                <span><strong>{syncProposal.source.extractedCount}</strong> catalogue assets read</span>
                <span><strong>{syncProposal.source.packageCount}</strong> installed packages indexed</span>
                <button className="button ghost" onClick={() => syncFromGame(true)}><FolderOpen size={15} /> Change game folder</button>
              </div>
              {!!syncProposal.source.warnings.length && <div className="sync-warnings"><CircleAlert size={17} /><div><strong>{syncProposal.source.warnings.length} extraction warnings</strong>{syncProposal.source.warnings.slice(0, 3).map((warning) => <span key={warning}>{warning}</span>)}</div></div>}
              <div className="sync-section-tabs">
                {syncProposal.sections.map((section) => <button className={syncSection === section.kind ? 'active' : ''} key={section.kind} onClick={() => setSyncSection(section.kind)}><span>{section.kind}</span><strong>{section.changes.length}</strong></button>)}
              </div>
              <div className="sync-controls">
                <div><strong>{selectedSyncIds.length} selected</strong><span>Missing and unsupported entries are never applied or deleted.</span></div>
                <button className="button ghost" onClick={() => setSelectedSyncIds((current) => Array.from(new Set([...current, ...visibleSyncChanges.filter((change) => change.selectedByDefault).map((change) => change.id)])))}>Select safe</button>
                <button className="button ghost" onClick={() => setSelectedSyncIds((current) => current.filter((id) => !visibleSyncChanges.some((change) => change.id === id)))}>Clear section</button>
              </div>
              <div className="sync-change-list">
                {visibleSyncChanges.length === 0 && <div className="empty-state"><Check size={32} /><strong>No differences in this section</strong><span>The installed game and local data agree.</span></div>}
                {visibleSyncChanges.map((change) => {
                  const checked = selectedSyncIds.includes(change.id);
                  return <label className={`sync-change ${checked ? 'selected' : ''} ${!change.canApply ? 'diagnostic' : ''}`} key={change.id}>
                    <input type="checkbox" checked={checked} disabled={!change.canApply} onChange={(event) => setSelectedSyncIds((current) => event.target.checked ? [...current, change.id] : current.filter((id) => id !== change.id))} />
                    <div className="sync-change-body">
                      <div className="sync-change-title">{change.iconDataUrl && <img className="sync-item-icon" src={change.iconDataUrl} alt="" />}<span className={`sync-action ${change.action}`}>{change.action}</span><strong>{change.displayName}</strong>{change.iconAsset && !change.iconDataUrl && <span className="sync-icon-found" title={change.iconAsset}>icon asset</span>}</div>
                      {change.assetName && <code>{change.assetName}</code>}
                      <p>{change.summary}</p>
                      {change.reason && <small>{change.reason}</small>}
                    </div>
                  </label>;
                })}
              </div>
              <div className="sync-footer">
                <p>Only the portable CSV and mapping files are changed. The game installation and save remain read-only.</p>
                <button className="button secondary" onClick={() => setSyncProposal(null)}>Cancel</button>
                <button className="button primary" disabled={selectedSyncIds.length === 0 || !!busy} onClick={applyGameSync}><Save size={16} /> Apply {selectedSyncIds.length || ''} selected</button>
              </div>
            </section>
          </div>
        )}
      </main>
    </div>
  );
}
