'use client';

import { useMemo, useState } from 'react';
import { ALL_STATS, DATA_COUNTS, FOOD_STATS, HUMANS, INITIAL_ITEMS, MEMORY_STATS, type GrowthItem, type Stat } from './game-data';
import { calculateRecipe, type Objective, type RecipeResult } from './optimizer';
import { importSave, type InventorySource, type SaveImportResult } from './save-parser';

const STAT_LABELS: Record<Stat, string> = {
  Weight: 'Weight', Height: 'Height', 'Life Exp': 'Life expectancy', Strength: 'Strength', Intellect: 'Intellect',
  Adaptability: 'Adaptability', Communication: 'Communication', Creativity: 'Creativity', Discipline: 'Discipline',
  Empathy: 'Empathy', Focus: 'Focus', Leadership: 'Leadership', Logic: 'Logic', Patience: 'Patience', Wisdom: 'Wisdom',
};

type Filter = 'owned' | 'food' | 'memory' | 'all';
const categories = [...new Set(HUMANS.map((human) => human.category))];

function cloneItems() {
  return INITIAL_ITEMS.map((item) => ({ ...item, stats: { ...item.stats } }));
}

function csvCell(value: string | number) {
  const text = String(value);
  return /[;"\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

export default function Home() {
  const [items, setItems] = useState<GrowthItem[]>(cloneItems);
  const [quantities, setQuantities] = useState<Record<string, number>>({});
  const [humanName, setHumanName] = useState(HUMANS[0].name);
  const [objective, setObjective] = useState<Objective>('waste');
  const [filter, setFilter] = useState<Filter>('owned');
  const [search, setSearch] = useState('');
  const [result, setResult] = useState<RecipeResult | null>(null);
  const [imported, setImported] = useState<SaveImportResult | null>(null);
  const [selectedSources, setSelectedSources] = useState<string[]>([]);
  const [fileName, setFileName] = useState('Select a .sav file');
  const [status, setStatus] = useState('The save is processed only in this browser.');
  const [error, setError] = useState('');
  const human = HUMANS.find((candidate) => candidate.name === humanName) ?? HUMANS[0];

  const visibleItems = useMemo(() => items.filter((item) => {
    const matchesFilter = filter === 'all' || filter === item.type || (filter === 'owned' && (quantities[item.id] ?? 0) > 0);
    return matchesFilter && item.name.toLowerCase().includes(search.toLowerCase());
  }), [items, quantities, filter, search]);

  const ownedStacks = items.filter((item) => (quantities[item.id] ?? 0) > 0).length;
  const ownedUnits = items.reduce((sum, item) => sum + (quantities[item.id] ?? 0), 0);
  const requirements = ALL_STATS.filter((stat) => human.requirements[stat] > 0);
  const unresolved = items.filter((item) => item.id.startsWith('unknown:') && (quantities[item.id] ?? 0) > 0);

  function quantitiesForSources(sourceIds: string[], sources: InventorySource[], catalogue: GrowthItem[]) {
    const next: Record<string, number> = Object.fromEntries(catalogue.map((item) => [item.id, 0]));
    const byName = new Map(catalogue.map((item) => [item.name, item.id]));
    for (const source of sources.filter((candidate) => sourceIds.includes(candidate.id))) {
      for (const importedItem of source.items) {
        const id = importedItem.mappedName ? byName.get(importedItem.mappedName) : `unknown:${importedItem.assetName}`;
        if (id) next[id] = (next[id] ?? 0) + importedItem.quantity;
      }
    }
    return next;
  }

  async function handleSave(file?: File) {
    if (!file) return;
    setFileName(file.name);
    setError('');
    setResult(null);
    try {
      const parsed = await importSave(file, setStatus);
      const unknownAssets = [...new Set(parsed.sources.flatMap((source) => source.items.filter((item) => !item.mappedName).map((item) => item.assetName)))];
      const catalogue = [...items.filter((item) => !item.id.startsWith('unknown:'))];
      for (const assetName of unknownAssets) {
        catalogue.push({
          id: `unknown:${assetName}`,
          name: `${assetName.replace(/^DA_(?:Food|Memory)_/, '').replaceAll('_', ' ')} · unmapped`,
          type: assetName.startsWith('DA_Food_') ? 'food' : 'memory',
          stats: Object.fromEntries(ALL_STATS.map((stat) => [stat, 0])) as Record<Stat, number>,
          availability: 0,
        });
      }
      const sourceIds = parsed.sources.map((source) => source.id);
      setItems(catalogue);
      setImported(parsed);
      setSelectedSources(sourceIds);
      setQuantities(quantitiesForSources(sourceIds, parsed.sources, catalogue));
      setFilter('owned');
      setStatus(`${parsed.sources.length} safe source(s) · ${parsed.blockCount} blocks processed`);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'The save could not be read.');
      setStatus('Import stopped');
    }
  }

  function toggleSource(sourceId: string) {
    if (!imported) return;
    const next = selectedSources.includes(sourceId) ? selectedSources.filter((id) => id !== sourceId) : [...selectedSources, sourceId];
    setSelectedSources(next);
    setQuantities(quantitiesForSources(next, imported.sources, items));
    setResult(null);
  }

  function updateItem(id: string, patch: Partial<GrowthItem>) {
    setItems((current) => current.map((item) => item.id === id ? { ...item, ...patch } : item));
    setResult(null);
  }

  function updateStat(id: string, stat: Stat, value: number) {
    setItems((current) => current.map((item) => item.id === id ? { ...item, stats: { ...item.stats, [stat]: Math.max(0, value || 0) } } : item));
    setResult(null);
  }

  function mapUnknown(id: string, targetId: string) {
    const target = items.find((item) => item.id === targetId);
    if (target) updateItem(id, { name: `${target.name} · manual assignment`, type: target.type, stats: { ...target.stats } });
  }

  function calculate() {
    setResult(calculateRecipe(items, quantities, human, objective));
    requestAnimationFrame(() => document.getElementById('recipe')?.scrollIntoView({ behavior: 'smooth', block: 'start' }));
  }

  function applyRecipe() {
    if (!result?.feasible) return;
    setQuantities((current) => Object.fromEntries(Object.entries(current).map(([id, quantity]) => [id, Math.max(0, quantity - (result.picks[id] ?? 0))])));
    setResult(null);
  }

  function resetData() {
    setItems(cloneItems());
    setQuantities({});
    setImported(null);
    setSelectedSources([]);
    setResult(null);
    setFileName('Select a .sav file');
    setStatus('CSV data restored.');
  }

  function exportInventory() {
    const header = ['Type', 'Name', 'Quantity', ...ALL_STATS];
    const lines = [header.join(';'), ...items.map((item) => [item.type, item.name, quantities[item.id] ?? 0, ...ALL_STATS.map((stat) => item.stats[stat])].map(csvCell).join(';'))];
    const url = URL.createObjectURL(new Blob([lines.join('\n')], { type: 'text/csv;charset=utf-8' }));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = 'caretaker-inventory.csv';
    anchor.click();
    URL.revokeObjectURL(url);
  }

  return (
    <main>
      <header className="topbar">
        <div className="brand"><span className="brand-mark">TLC</span><span>CARETAKER LAB</span></div>
        <nav><a href="#planner">PLANNER</a><a href="#inventory">INVENTORY</a><a href="#method">METHOD</a></nav>
        <span className="privacy">LOCAL · NO UPLOADS</span>
      </header>

      <section className="hero">
        <div className="hero-copy">
          <p className="eyebrow">HUMAN GROWTH / INVENTORY PLANNER</p>
          <h1>Grow the right human.<br /><em>Without burning the future.</em></h1>
          <p className="lede">Import the backpack and player chests, correct any local data, and find the recipe with the least excess.</p>
          <div className="data-line"><span>{DATA_COUNTS.food} FOODS</span><span>{DATA_COUNTS.memories} MEMORIES</span><span>{DATA_COUNTS.professions} PROFESSIONS</span><span>CSV 25 AUG 2026</span></div>
        </div>
        <label className="import-card">
          <input type="file" accept=".sav" onChange={(event) => handleSave(event.target.files?.[0])} />
          <span className="import-kicker">IMPORT SAVE</span>
          <strong>{fileName}</strong>
          <span className="import-action">Read safe inventory →</span>
          <small>{status}</small>
          {error && <span className="error-text">{error}</span>}
        </label>
      </section>

      {imported && (
        <section className="sources-strip">
          <div><p className="section-label">VERIFIED SOURCES</p><strong>The global world inventory is never used.</strong></div>
          <div className="source-list">
            {imported.sources.map((source) => (
              <label key={source.id} className={selectedSources.includes(source.id) ? 'source active' : 'source'}>
                <input type="checkbox" checked={selectedSources.includes(source.id)} onChange={() => toggleSource(source.id)} />
                <span>{source.kind === 'backpack' ? 'BACKPACK' : 'CHEST'}</span>
                <strong>{source.label.replace(/^Player chest · /, '')}</strong>
                <small>{source.items.reduce((sum, item) => sum + item.quantity, 0)} growth resources</small>
              </label>
            ))}
          </div>
          {!imported.customLabelFound && <p className="notice">The <b>PruebaItems001</b> label is not present in this save. Chests are identified by player class and internal ID.</p>}
        </section>
      )}

      <section className="workspace" id="planner">
        <aside className="target-panel">
          <p className="section-label">01 / TARGET</p>
          <h2>Who do you need?</h2>
          <label className="select-label">Profession
            <select value={humanName} onChange={(event) => { setHumanName(event.target.value); setResult(null); }}>
              {categories.map((category) => <optgroup label={category} key={category}>{HUMANS.filter((candidate) => candidate.category === category).map((candidate) => <option key={candidate.name}>{candidate.name}</option>)}</optgroup>)}
            </select>
          </label>
          <div className="requirements">
            {requirements.map((stat) => <span key={stat}>{STAT_LABELS[stat].toUpperCase()} <b>{human.requirements[stat]}</b></span>)}
          </div>
          <fieldset className="objective-choice">
            <legend>Priority</legend>
            <label><input type="radio" checked={objective === 'waste'} onChange={() => setObjective('waste')} /> Minimum excess</label>
            <label><input type="radio" checked={objective === 'items'} onChange={() => setObjective('items')} /> Minimum items</label>
          </fieldset>
          <button className="primary" onClick={calculate}>CALCULATE OPTIMAL RECIPE</button>
          <p className="target-note">Availability limits the calculation. Empty CSV values count as zero.</p>
        </aside>

        <section className="inventory-panel" id="inventory">
          <div className="inventory-head">
            <div><p className="section-label">02 / INVENTORY</p><h2>Available resources</h2></div>
            <span className="count-chip">{ownedUnits} UNITS · {ownedStacks} TYPES</span>
          </div>
          <div className="inventory-tools">
            <div className="tabs">
              {(['owned', 'food', 'memory', 'all'] as Filter[]).map((value) => <button className={filter === value ? 'tab active' : 'tab'} onClick={() => setFilter(value)} key={value}>{value === 'owned' ? 'OWNED' : value === 'food' ? 'FOOD' : value === 'memory' ? 'MEMORIES' : 'ALL'}</button>)}
            </div>
            <input className="search" placeholder="Search items…" value={search} onChange={(event) => setSearch(event.target.value)} />
          </div>
          {unresolved.length > 0 && <div className="warning"><b>{unresolved.length} asset(s) without an exact match.</b> Open them and assign a CSV item or edit their statistics.</div>}
          <div className="inventory-grid">
            {visibleItems.length === 0 && <div className="empty">There are no items in this view. Import a save or change the filter.</div>}
            {visibleItems.map((item, index) => {
              const stats = item.type === 'food' ? FOOD_STATS : MEMORY_STATS;
              const active = stats.filter((stat) => item.stats[stat] > 0);
              return (
                <details className="item-row" key={item.id}>
                  <summary>
                    <span className="item-index">{String(index + 1).padStart(2, '0')}</span>
                    <div><strong>{item.name}</strong><small>{item.type === 'food' ? 'Food' : 'Memory'} · {active.map((stat) => `+${item.stats[stat]} ${STAT_LABELS[stat]}`).join(' · ') || 'no statistics'}</small></div>
                    <input aria-label={`Quantity of ${item.name}`} type="number" min="0" value={quantities[item.id] ?? 0} onClick={(event) => event.stopPropagation()} onChange={(event) => { setQuantities((current) => ({ ...current, [item.id]: Math.max(0, Number(event.target.value) || 0) })); setResult(null); }} />
                    <span className="chevron">＋</span>
                  </summary>
                  <div className="item-editor">
                    <label>Name<input value={item.name} onChange={(event) => updateItem(item.id, { name: event.target.value })} /></label>
                    {item.id.startsWith('unknown:') && <label>Copy data from<select defaultValue="" onChange={(event) => mapUnknown(item.id, event.target.value)}><option value="">Select an item…</option>{items.filter((candidate) => candidate.type === item.type && !candidate.id.startsWith('unknown:')).map((candidate) => <option value={candidate.id} key={candidate.id}>{candidate.name}</option>)}</select></label>}
                    <div className="stat-editor">{stats.map((stat) => <label key={stat}>{STAT_LABELS[stat]}<input type="number" min="0" value={item.stats[stat]} onChange={(event) => updateStat(item.id, stat, Number(event.target.value))} /></label>)}</div>
                  </div>
                </details>
              );
            })}
          </div>
          <div className="inventory-actions"><button onClick={exportInventory}>EXPORT INVENTORY CSV</button><button onClick={resetData}>RESTORE BASE DATA</button></div>
        </section>
      </section>

      {result && (
        <section className={result.feasible ? 'recipe success' : 'recipe failure'} id="recipe">
          <div className="recipe-head">
            <div><p className="section-label">03 / RESULT</p><h2>{result.feasible ? 'Feasible recipe' : 'Insufficient inventory'}</h2></div>
            <div className="score"><span>{result.itemCount}<small>ITEMS</small></span><span>{result.waste}<small>EXCESS</small></span></div>
          </div>
          {result.feasible ? (
            <div className="recipe-layout">
              <div className="picks">{Object.entries(result.picks).filter(([, quantity]) => quantity > 0).map(([id, quantity]) => <article key={id}><b>{quantity}×</b><span>{items.find((item) => item.id === id)?.name}</span></article>)}</div>
              <div className="stat-result">{requirements.map((stat) => <div key={stat}><span>{STAT_LABELS[stat]}</span><b>{result.totals[stat]} / {human.requirements[stat]}</b><i>+{result.excess[stat]}</i></div>)}</div>
            </div>
          ) : (
            <div className="deficits"><p>The best available combination still has these deficits:</p>{requirements.filter((stat) => result.deficits[stat] > 0).map((stat) => <span key={stat}>{STAT_LABELS[stat]} <b>−{result.deficits[stat]}</b></span>)}</div>
          )}
          {result.feasible && <button className="apply" onClick={applyRecipe}>SUBTRACT THIS RECIPE FROM INVENTORY</button>}
        </section>
      )}

      <section className="method" id="method">
        <p className="section-label">METHOD / LIMITS</p>
        <div><h2>Safe by design.</h2><p>Import recognizes the character backpack and only <code>BP_Inventory_PlayerBox</code> actors. Environmental containers and the global inventory remain excluded. The reader corrects the mirrored copy stored inside a chest only when both sequences match exactly.</p></div>
        <div><h2>Data under your control.</h2><p>Statistics come from the three supplied CSV files. You can edit any value, quantity, or assignment without touching the saved game. The calculator never writes the <code>.sav</code> file.</p></div>
      </section>

      <footer><span>CARETAKER LAB · UNOFFICIAL TOOL</span><span>THE LAST CARETAKER™ BELONGS TO ITS RESPECTIVE OWNERS</span></footer>
    </main>
  );
}
