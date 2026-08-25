export const FOOD_STATS = ['Height', 'Intellect', 'Life Exp', 'Strength', 'Weight'] as const;
export const MEMORY_STATS = ['Adaptability', 'Communication', 'Creativity', 'Discipline', 'Empathy', 'Focus', 'Leadership', 'Logic', 'Patience', 'Wisdom'] as const;
export const ALL_STATS = ['Weight', 'Height', 'Life Exp', 'Strength', 'Intellect', ...MEMORY_STATS] as const;

export type Stat = (typeof ALL_STATS)[number];
export type GrowthType = 'food' | 'memory';

export type GrowthItem = {
  id: string;
  name: string;
  type: GrowthType;
  stats: Record<Stat, number>;
  availability: number;
};

export type Human = {
  category: string;
  name: string;
  requirements: Record<Stat, number>;
};

const FOOD_CSV = `Food;Height;Intellect;Life Exp;Strength;Weight;TotalAvailability
High-Fat;1;1;1;1;8;313
Mind Surge;1;6;6;1;1;45
Nutri-Core;3;3;3;3;3;67
Physique Fuel;5;1;1;5;1;14
Bone-Fortify;8;2;2;14;2;76
Endura-Growth;2;12;8;2;2;78
Immune Boost;6;6;6;6;6;76
Muscle Fortification;2;2;2;15;8;38
Neuro-Boost;2;15;10;2;2;35
Hyper-Evolution;30;20;5;3;3;88
Mitochondrial Surge;20;3;12;28;3;88
Nanite Infusion;25;25;6;3;3;76
Ultimate Genesis;50;50;40;10;10;88
Pear;1;1;1;1;1;300`;

const MEMORY_CSV = `Memory;Adaptability;Communication;Creativity;Discipline;Empathy;Focus;Leadership;Logic;Patience;Wisdom;WorldCount
Angerkoski;20;;25;;;25;;;20;;30
Ash Notebook;100;100;100;100;100;100;100;100;100;100;10
Assembly Instructions;;;;2;;;;8;;;30
Basketball;;;;;;;;;1;;36
Biology Notes;;;;;;;;;;10;20
Blueprints;10;;;;;;;5;;;22
Bowling Ball;;;;;;;;;1;;16
Bowling Pin;;;;;;;;;1;;54
Breakdown;20;;25;;;25;;;20;;30
Camera;;5;5;;;;;;;;29
Cards;;5;;;5;;;;;;31
Chiptropolis;20;;25;;;25;;;20;;30
Cognitive Cards;7;;;;;;;5;;3;19
Commander's Log;;;;7;;;8;;;;31
Compass;6;;;;;3;3;;;;19
Crayon;;;5;;;2;;;;;23
Encyclopedia;;;;;;;;5;;10;21
First Aid;10;;;;;;;;;;15
Fluctuations;20;;25;;;25;;;20;;30
Guitar;;3;3;;;3;;;;;31
Guitar Slinger;20;;25;;;25;;;20;;30
Keys to Imagination;20;;25;;;25;;;20;;30
Lonelies;20;;25;;;25;;;20;;30
Love Letters;;;;;;;;;;10;15
Maps;;;;;;;5;;;10;26
Meditation;;;;;;5;;;;;19
Mirror;;;;;5;;;;;;22
Music Notes;;4;6;;;;;;;;16
Mystery Box;;;;;;;;;5;;23
Oath Token;50;50;50;50;50;50;50;50;50;50;1
PECO Athletics - 100m Dash;20;;25;;;25;;;20;;30
PECO Athletics - Javelin;20;;25;;;25;;;20;;30
PECO Athletics - Triple Jump;20;;25;;;25;;;20;;30
Plans;;8;;7;;;;;;;18
Porcine Vocal Interface;;100;50;;50;;;;;;10
Poushki;20;;25;;;25;;;20;;30
Programming Manual;;;;;;;;10;;;22
Second Reality;20;;25;;;25;;;20;;30
Small Human Art;;;;;5;;;;;;31
Small Tree;;;;5;;;;;5;;25
Starshine;20;;25;;;25;;;20;;30
Stopwatch;;;;;;;;;5;;23
Sudoku Book;;;;;;;;10;;;18
Survival Diagrams;10;;;;;;5;;;;14
Teddy Bear;;;;;3;;;;;;17
The Art of War;;5;;;;;10;;;;24
Tommy;;;;;3;;;;;;17
Touchin' the Chip;20;;25;;;25;;;20;;30
Travel Journal;10;;;;;;;;;;13
Turbulence;20;;25;;;25;;;20;;30
Where's Tommy;;;;;;5;;;5;;16`;

const HUMANS_CSV = `Category;Profession;Weight;Height;Life Exp;Strength;Intellect;Adaptability;Creativity;Communication;Discipline;Empathy;Focus;Leadership;Logic;Patience;Wisdom
Engineer;Maintenance Engineer T1;20;30;10;50;;;;;5;;4;;3;;
Engineer;Systems Engineer T2;20;30;10;70;;;;;10;;10;;10;;
Engineer;Energy Engineer T3;20;30;10;120;;;;;30;;30;;30;;
Engineer;Quantum Engineer T4;20;30;10;180;;;;;60;;80;;80;;
Arts & Culture;Visual Technician T1;20;30;50;;;;5;4;;3;;;;;
Arts & Culture;Sculptor T2;20;30;80;;;;20;10;;15;;;;;
Arts & Culture;Cultural Archivist T3;20;30;140;;;;45;35;;40;;;;;
Arts & Culture;Existential Expressionist T4;20;30;200;;;;120;60;;80;;;;;
Educator;Manual Holder T1;40;100;10;;;;;5;;;;;;10;10
Educator;Teacher T2;60;160;10;;;;;15;;;;;;12;10
Educator;Professor T3;90;180;10;;;;;20;;;;;;30;60
Educator;Existential Chancellor T4;120;200;10;;;;;100;;;;;;80;120
Agriculture;Nutrient Handler T1;20;160;50;;;;4;;;;;3;;5;
Agriculture;Growth Specialist T2;20;180;60;;;;20;;;;;10;;15;
Agriculture;Biosphere Director T3;20;190;120;;;;35;;;;;30;;40;
Agriculture;Sustenance Architect T4;20;200;180;;;;50;;;;;50;;160;
Logistics;Basic Supplier T1;20;100;10;;;;;;3;;4;;2;;
Logistics;Distributor T2;20;160;10;;;;;;12;;15;;10;;
Logistics;Resource Director T3;20;190;10;;;;;;35;;40;;30;;
Logistics;Logistics High Command T4;20;220;10;;;;;;80;;100;;120;;
Military;Door Jammer T1;20;30;10;60;;;;;2;;2;5;;;
Military;Guard T2;20;30;10;120;;;;;15;;12;10;;;
Military;Station Protector T3;20;180;10;130;;;;;40;;35;30;;;
Military;Guardian of Humanity T4;20;200;10;200;;;;;100;;120;80;;;
Science;Lab Technician T1;20;30;10;;80;;;;;;;;5;5;3
Science;Field Research Scientist T2;20;30;10;;110;;;;;;;;9;10;9
Science;Theoretical Scientist T3;20;30;10;;170;;;;;;;;25;30;40
Science;Quantum Physicist T4;20;30;10;;220;;;;;;;;120;80;100
Healthcare;Health Assistant T1;50;30;10;;;;;;;5;;;;4;3
Healthcare;Doctor T2;80;30;10;;;;;;;15;;;;12;10
Healthcare;Neuro Specialist T3;120;30;10;;;;;;;40;;;;35;30
Healthcare;Neural Architect T4;200;30;10;;;;;;;100;;;;80;120
Leadership;Room Supervisor T1;20;30;40;;;;;4;;3;;5;;;
Leadership;Station Quartermaster T2;20;30;70;;;;;10;;10;;15;;;
Leadership;Settlement Governor T3;20;30;100;;;;;35;;30;;40;;;
Leadership;Colonel of Humanity T4;20;30;220;;;;;100;;80;;120;;;
Explorer;Station Roamer T1;20;30;20;60;;2;1;;;;3;;;;
Explorer;Star Analyzer T2;20;30;60;100;;15;20;;;;10;;;;
Explorer;Mission Seeker T3;20;30;90;100;;40;35;;;;30;;;;
Explorer;Frontier Explorer T4;20;30;140;120;;100;80;;;;120;;;;`;

function rows(csv: string) {
  const [header, ...lines] = csv.trim().split(/\r?\n/);
  const columns = header.split(';');
  return lines.map((line) => Object.fromEntries(line.split(';').map((value, index) => [columns[index], value])));
}

function emptyStats(): Record<Stat, number> {
  return Object.fromEntries(ALL_STATS.map((stat) => [stat, 0])) as Record<Stat, number>;
}

export const INITIAL_ITEMS: GrowthItem[] = [
  ...rows(FOOD_CSV).map((row) => ({
    id: `food:${row.Food}`,
    name: row.Food,
    type: 'food' as const,
    stats: { ...emptyStats(), ...Object.fromEntries(FOOD_STATS.map((stat) => [stat, Number(row[stat] || 0)])) },
    availability: Number(row.TotalAvailability || 0),
  })),
  ...rows(MEMORY_CSV).map((row) => ({
    id: `memory:${row.Memory}`,
    name: row.Memory,
    type: 'memory' as const,
    stats: { ...emptyStats(), ...Object.fromEntries(MEMORY_STATS.map((stat) => [stat, Number(row[stat] || 0)])) },
    availability: Number(row.WorldCount || 0),
  })),
];

export const HUMANS: Human[] = rows(HUMANS_CSV).map((row) => ({
  category: row.Category,
  name: row.Profession,
  requirements: Object.fromEntries(ALL_STATS.map((stat) => [stat, Number(row[stat] || 0)])) as Record<Stat, number>,
}));

export const DATA_COUNTS = { food: 14, memories: 51, professions: 40 };
