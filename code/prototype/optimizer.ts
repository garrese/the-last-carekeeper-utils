import { ALL_STATS, FOOD_STATS, MEMORY_STATS, type GrowthItem, type Human, type Stat } from './game-data';

export type Objective = 'waste' | 'items';

export type RecipeResult = {
  feasible: boolean;
  picks: Record<string, number>;
  totals: Record<Stat, number>;
  deficits: Record<Stat, number>;
  excess: Record<Stat, number>;
  itemCount: number;
  waste: number;
};

type Node = { actual: number[]; picks: Record<string, number>; count: number };

function better(a: Node, b: Node | undefined, required: number[], objective: Objective) {
  if (!b) return true;
  const wasteA = a.actual.reduce((sum, value, index) => sum + Math.max(0, value - required[index]), 0);
  const wasteB = b.actual.reduce((sum, value, index) => sum + Math.max(0, value - required[index]), 0);
  return objective === 'waste' ? wasteA < wasteB || (wasteA === wasteB && a.count < b.count) : a.count < b.count || (a.count === b.count && wasteA < wasteB);
}

function optimizeSection(items: GrowthItem[], quantities: Record<string, number>, human: Human, stats: readonly Stat[], objective: Objective) {
  const activeStats = stats.filter((stat) => human.requirements[stat] > 0);
  if (!activeStats.length) return { node: { actual: [], picks: {}, count: 0 }, activeStats, feasible: true };
  const required = activeStats.map((stat) => human.requirements[stat]);
  let states = new Map<string, Node>([['0,'.repeat(activeStats.length).slice(0, -1), { actual: activeStats.map(() => 0), picks: {}, count: 0 }]]);

  for (const item of items) {
    const available = Math.max(0, Math.floor(quantities[item.id] ?? 0));
    const contribution = activeStats.map((stat) => item.stats[stat]);
    if (!available || contribution.every((value) => value <= 0)) continue;
    const usefulCopies = Math.max(...contribution.map((value, index) => value > 0 ? Math.ceil(required[index] / value) : 0));
    const copies = Math.min(available, Math.max(1, usefulCopies));
    for (let copy = 0; copy < copies; copy += 1) {
      const next = new Map(states);
      for (const node of states.values()) {
        const actual = node.actual.map((value, index) => value + contribution[index]);
        const capped = actual.map((value, index) => Math.min(value, required[index]));
        const key = capped.join(',');
        const candidate = { actual, picks: { ...node.picks, [item.id]: (node.picks[item.id] ?? 0) + 1 }, count: node.count + 1 };
        if (better(candidate, next.get(key), required, objective)) next.set(key, candidate);
      }
      states = next;
    }
  }

  const target = required.join(',');
  const exact = states.get(target);
  if (exact) return { node: exact, activeStats, feasible: true };
  const fallback = [...states.values()].sort((a, b) => {
    const progress = (node: Node) => node.actual.reduce((sum, value, index) => sum + Math.min(value, required[index]) / required[index], 0);
    return progress(b) - progress(a) || a.count - b.count;
  })[0];
  return { node: fallback, activeStats, feasible: false };
}

export function calculateRecipe(items: GrowthItem[], quantities: Record<string, number>, human: Human, objective: Objective): RecipeResult {
  const food = optimizeSection(items.filter((item) => item.type === 'food'), quantities, human, FOOD_STATS, objective);
  const memories = optimizeSection(items.filter((item) => item.type === 'memory'), quantities, human, MEMORY_STATS, objective);
  const picks: Record<string, number> = { ...food.node.picks };
  for (const [id, quantity] of Object.entries(memories.node.picks)) picks[id] = (picks[id] ?? 0) + quantity;
  const totals = Object.fromEntries(ALL_STATS.map((stat) => [stat, 0])) as Record<Stat, number>;
  for (const [id, quantity] of Object.entries(picks)) {
    const item = items.find((candidate) => candidate.id === id);
    if (!item) continue;
    for (const stat of ALL_STATS) totals[stat] += item.stats[stat] * quantity;
  }
  const deficits = Object.fromEntries(ALL_STATS.map((stat) => [stat, Math.max(0, human.requirements[stat] - totals[stat])])) as Record<Stat, number>;
  const excess = Object.fromEntries(ALL_STATS.map((stat) => [stat, Math.max(0, totals[stat] - human.requirements[stat])])) as Record<Stat, number>;
  return {
    feasible: food.feasible && memories.feasible,
    picks,
    totals,
    deficits,
    excess,
    itemCount: Object.values(picks).reduce((sum, quantity) => sum + quantity, 0),
    waste: ALL_STATS.reduce((sum, stat) => sum + excess[stat], 0),
  };
}
