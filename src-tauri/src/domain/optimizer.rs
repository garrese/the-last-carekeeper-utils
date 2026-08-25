use super::{
    CatalogueBundle, CsvDocument, FOOD_STATS, HumanAssessment, MEMORY_STATS, RecipePick,
    RecipeResult, STATS,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone)]
struct GrowthItem {
    name: String,
    item_type: String,
    stats: BTreeMap<String, i32>,
}

#[derive(Debug, Clone)]
struct Human {
    category: String,
    profession: String,
    requirements: BTreeMap<String, i32>,
}

#[derive(Debug, Clone)]
struct Node {
    actual: Vec<i32>,
    picks: BTreeMap<usize, u32>,
    count: u32,
}

struct SectionResult {
    node: Node,
    feasible: bool,
}

fn column(document: &CsvDocument, name: &str) -> Result<usize, String> {
    document
        .headers
        .iter()
        .position(|header| header == name)
        .ok_or_else(|| format!("Missing {name} column in {}.", document.file_name))
}

fn cell_number(row: &[String], index: usize) -> i32 {
    row.get(index)
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

fn growth_items(catalogues: &CatalogueBundle) -> Result<Vec<GrowthItem>, String> {
    let mut items = Vec::new();
    for (document, name_column, item_type, stats) in [
        (&catalogues.food, "Food", "food", FOOD_STATS.as_slice()),
        (
            &catalogues.memories,
            "Memory",
            "memory",
            MEMORY_STATS.as_slice(),
        ),
    ] {
        let name_index = column(document, name_column)?;
        let stat_indices = stats
            .iter()
            .map(|stat| column(document, stat).map(|index| ((*stat).to_string(), index)))
            .collect::<Result<Vec<_>, _>>()?;
        for row in &document.rows {
            let mut values = BTreeMap::new();
            for (stat, index) in &stat_indices {
                values.insert(stat.clone(), cell_number(row, *index));
            }
            items.push(GrowthItem {
                name: row[name_index].trim().to_string(),
                item_type: item_type.to_string(),
                stats: values,
            });
        }
    }
    Ok(items)
}

fn humans(document: &CsvDocument) -> Result<Vec<Human>, String> {
    let category_index = column(document, "Category")?;
    let profession_index = column(document, "Profession")?;
    let stat_indices = STATS
        .iter()
        .map(|stat| column(document, stat).map(|index| ((*stat).to_string(), index)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(document
        .rows
        .iter()
        .map(|row| {
            let requirements = stat_indices
                .iter()
                .map(|(stat, index)| (stat.clone(), cell_number(row, *index)))
                .collect();
            Human {
                category: row[category_index].trim().to_string(),
                profession: row[profession_index].trim().to_string(),
                requirements,
            }
        })
        .collect())
}

fn available_totals(
    items: &[GrowthItem],
    inventory: &BTreeMap<String, u32>,
) -> BTreeMap<String, i32> {
    let mut totals = STATS
        .iter()
        .map(|stat| ((*stat).to_string(), 0))
        .collect::<BTreeMap<_, _>>();
    for item in items {
        let quantity = i32::try_from(*inventory.get(&item.name).unwrap_or(&0)).unwrap_or(i32::MAX);
        for (stat, value) in &item.stats {
            *totals.entry(stat.clone()).or_default() += value.saturating_mul(quantity);
        }
    }
    totals
}

pub fn assess_humans(
    catalogues: &CatalogueBundle,
    inventory: &BTreeMap<String, u32>,
) -> Result<Vec<HumanAssessment>, String> {
    let items = growth_items(catalogues)?;
    let totals = available_totals(&items, inventory);
    let mut assessments = humans(&catalogues.humans)?
        .into_iter()
        .map(|human| {
            let deficits = STATS
                .iter()
                .map(|stat| {
                    let required = *human.requirements.get(*stat).unwrap_or(&0);
                    let available = *totals.get(*stat).unwrap_or(&0);
                    ((*stat).to_string(), (required - available).max(0))
                })
                .collect::<BTreeMap<_, _>>();
            let required_stats = human
                .requirements
                .iter()
                .filter(|(_, required)| **required > 0)
                .collect::<Vec<_>>();
            let coverage_percent = if required_stats.is_empty() {
                100.0
            } else {
                required_stats
                    .iter()
                    .map(|(stat, required)| {
                        let available = f64::from(*totals.get(*stat).unwrap_or(&0));
                        (available / f64::from(**required)).min(1.0)
                    })
                    .sum::<f64>()
                    / required_stats.len() as f64
                    * 100.0
            };
            HumanAssessment {
                category: human.category,
                profession: human.profession,
                achievable: deficits.values().all(|deficit| *deficit == 0),
                coverage_percent,
                deficits,
            }
        })
        .collect::<Vec<_>>();
    assessments.sort_by(|left, right| {
        right
            .achievable
            .cmp(&left.achievable)
            .then_with(|| {
                right
                    .coverage_percent
                    .partial_cmp(&left.coverage_percent)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| left.profession.cmp(&right.profession))
    });
    Ok(assessments)
}

fn waste(node: &Node, required: &[i32]) -> i32 {
    node.actual
        .iter()
        .zip(required)
        .map(|(actual, required)| (actual - required).max(0))
        .sum()
}

fn is_better(candidate: &Node, current: Option<&Node>, required: &[i32], objective: &str) -> bool {
    let Some(current) = current else { return true };
    let candidate_waste = waste(candidate, required);
    let current_waste = waste(current, required);
    if objective == "items" {
        candidate.count < current.count
            || (candidate.count == current.count && candidate_waste < current_waste)
    } else {
        candidate_waste < current_waste
            || (candidate_waste == current_waste && candidate.count < current.count)
    }
}

fn optimize_section(
    items: &[GrowthItem],
    relevant_indices: &[usize],
    inventory: &BTreeMap<String, u32>,
    human: &Human,
    stat_names: &[&str],
    objective: &str,
) -> SectionResult {
    let active_stats = stat_names
        .iter()
        .filter(|stat| *human.requirements.get(**stat).unwrap_or(&0) > 0)
        .copied()
        .collect::<Vec<_>>();
    let required = active_stats
        .iter()
        .map(|stat| *human.requirements.get(*stat).unwrap_or(&0))
        .collect::<Vec<_>>();
    if active_stats.is_empty() {
        return SectionResult {
            node: Node {
                actual: Vec::new(),
                picks: BTreeMap::new(),
                count: 0,
            },
            feasible: true,
        };
    }
    let zero = vec![0; active_stats.len()];
    let mut states = HashMap::from([(
        zero,
        Node {
            actual: vec![0; active_stats.len()],
            picks: BTreeMap::new(),
            count: 0,
        },
    )]);

    for item_index in relevant_indices {
        let item = &items[*item_index];
        let available = *inventory.get(&item.name).unwrap_or(&0);
        let contribution = active_stats
            .iter()
            .map(|stat| *item.stats.get(*stat).unwrap_or(&0))
            .collect::<Vec<_>>();
        if available == 0 || contribution.iter().all(|value| *value <= 0) {
            continue;
        }
        let useful_copies = contribution
            .iter()
            .zip(&required)
            .filter(|(value, _)| **value > 0)
            .map(|(value, required)| (required + value - 1) / value)
            .max()
            .unwrap_or(0)
            .max(1);
        let copies = available.min(u32::try_from(useful_copies).unwrap_or(u32::MAX));
        for _ in 0..copies {
            let snapshot = states.values().cloned().collect::<Vec<_>>();
            let mut next = states;
            for node in snapshot {
                let actual = node
                    .actual
                    .iter()
                    .zip(&contribution)
                    .map(|(value, add)| value.saturating_add(*add))
                    .collect::<Vec<_>>();
                let capped = actual
                    .iter()
                    .zip(&required)
                    .map(|(value, requirement)| (*value).min(*requirement))
                    .collect::<Vec<_>>();
                let mut picks = node.picks.clone();
                *picks.entry(*item_index).or_default() += 1;
                let candidate = Node {
                    actual,
                    picks,
                    count: node.count + 1,
                };
                if is_better(&candidate, next.get(&capped), &required, objective) {
                    next.insert(capped, candidate);
                }
            }
            states = next;
        }
    }

    if let Some(node) = states.get(&required).cloned() {
        return SectionResult {
            node,
            feasible: true,
        };
    }
    let fallback = states
        .into_values()
        .max_by(|left, right| {
            let progress = |node: &Node| {
                node.actual
                    .iter()
                    .zip(&required)
                    .map(|(value, requirement)| {
                        f64::from((*value).min(*requirement)) / f64::from(*requirement)
                    })
                    .sum::<f64>()
            };
            progress(left)
                .partial_cmp(&progress(right))
                .unwrap_or(Ordering::Equal)
                .then_with(|| right.count.cmp(&left.count))
        })
        .unwrap_or(Node {
            actual: vec![0; required.len()],
            picks: BTreeMap::new(),
            count: 0,
        });
    SectionResult {
        node: fallback,
        feasible: false,
    }
}

pub fn calculate_recipe(
    catalogues: &CatalogueBundle,
    inventory: &BTreeMap<String, u32>,
    profession: &str,
    objective: &str,
) -> Result<RecipeResult, String> {
    if objective != "waste" && objective != "items" {
        return Err("Objective must be either waste or items.".to_string());
    }
    let items = growth_items(catalogues)?;
    let all_humans = humans(&catalogues.humans)?;
    let human = all_humans
        .iter()
        .find(|human| human.profession == profession)
        .ok_or_else(|| format!("Unknown profession: {profession}"))?;
    let food_indices = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| (item.item_type == "food").then_some(index))
        .collect::<Vec<_>>();
    let memory_indices = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| (item.item_type == "memory").then_some(index))
        .collect::<Vec<_>>();
    let food = optimize_section(
        &items,
        &food_indices,
        inventory,
        human,
        &FOOD_STATS,
        objective,
    );
    let memories = optimize_section(
        &items,
        &memory_indices,
        inventory,
        human,
        &MEMORY_STATS,
        objective,
    );
    let mut picks = food.node.picks;
    for (index, quantity) in memories.node.picks {
        *picks.entry(index).or_default() += quantity;
    }
    let mut totals = STATS
        .iter()
        .map(|stat| ((*stat).to_string(), 0))
        .collect::<BTreeMap<_, _>>();
    let recipe_picks = picks
        .iter()
        .map(|(index, quantity)| {
            let item = &items[*index];
            for (stat, value) in &item.stats {
                *totals.entry(stat.clone()).or_default() +=
                    value.saturating_mul(i32::try_from(*quantity).unwrap_or(i32::MAX));
            }
            RecipePick {
                item_name: item.name.clone(),
                item_type: item.item_type.clone(),
                quantity: *quantity,
            }
        })
        .collect::<Vec<_>>();
    let requirements = human.requirements.clone();
    let deficits = STATS
        .iter()
        .map(|stat| {
            let required = *requirements.get(*stat).unwrap_or(&0);
            let actual = *totals.get(*stat).unwrap_or(&0);
            ((*stat).to_string(), (required - actual).max(0))
        })
        .collect::<BTreeMap<_, _>>();
    let excess = STATS
        .iter()
        .map(|stat| {
            let required = *requirements.get(*stat).unwrap_or(&0);
            let actual = *totals.get(*stat).unwrap_or(&0);
            ((*stat).to_string(), (actual - required).max(0))
        })
        .collect::<BTreeMap<_, _>>();
    let matched_professions = all_humans
        .iter()
        .filter(|candidate| {
            candidate
                .requirements
                .iter()
                .all(|(stat, required)| totals.get(stat).copied().unwrap_or(0) >= *required)
        })
        .map(|candidate| candidate.profession.clone())
        .collect::<Vec<_>>();
    Ok(RecipeResult {
        profession: human.profession.clone(),
        feasible: food.feasible && memories.feasible,
        item_count: recipe_picks.iter().map(|pick| pick.quantity).sum(),
        waste: excess.values().sum(),
        picks: recipe_picks,
        totals,
        requirements,
        deficits,
        excess,
        matched_professions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(kind: &str, headers: &[&str], rows: &[&[&str]]) -> CsvDocument {
        CsvDocument {
            kind: kind.into(),
            file_name: format!("{kind}.csv"),
            headers: headers.iter().map(|value| (*value).into()).collect(),
            rows: rows
                .iter()
                .map(|row| row.iter().map(|value| (*value).into()).collect())
                .collect(),
        }
    }

    #[test]
    fn chooses_lower_waste_recipe() {
        let bundle = CatalogueBundle {
            food: document(
                "food",
                &[
                    "Food",
                    "Height",
                    "Intellect",
                    "Life Exp",
                    "Strength",
                    "Weight",
                    "TotalAvailability",
                ],
                &[
                    &["Balanced", "10", "0", "10", "0", "10", "1"],
                    &["Wasteful", "20", "0", "20", "0", "20", "1"],
                ],
            ),
            memories: document(
                "memories",
                &[
                    "Memory",
                    "Adaptability",
                    "Communication",
                    "Creativity",
                    "Discipline",
                    "Empathy",
                    "Focus",
                    "Leadership",
                    "Logic",
                    "Patience",
                    "Wisdom",
                    "WorldCount",
                ],
                &[&["None", "", "", "", "", "", "", "", "", "", "", "1"]],
            ),
            humans: document(
                "humans",
                &[
                    "Category",
                    "Profession",
                    "Weight",
                    "Height",
                    "Life Exp",
                    "Strength",
                    "Intellect",
                    "Adaptability",
                    "Creativity",
                    "Communication",
                    "Discipline",
                    "Empathy",
                    "Focus",
                    "Leadership",
                    "Logic",
                    "Patience",
                    "Wisdom",
                ],
                &[&[
                    "Test", "Target", "10", "10", "10", "", "", "", "", "", "", "", "", "", "", "",
                    "",
                ]],
            ),
        };
        let inventory = BTreeMap::from([("Balanced".into(), 1), ("Wasteful".into(), 1)]);
        let result = calculate_recipe(&bundle, &inventory, "Target", "waste").unwrap();
        assert!(result.feasible);
        assert_eq!(result.picks[0].item_name, "Balanced");
        assert_eq!(result.waste, 0);
    }

    #[test]
    fn mixed_zero_contributions_do_not_divide_by_zero() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let catalogues = crate::domain::catalog::load_all(root).unwrap();
        let inventory = catalogues
            .food
            .rows
            .iter()
            .chain(catalogues.memories.rows.iter())
            .map(|row| (row[0].clone(), 1))
            .collect::<BTreeMap<_, _>>();

        let result =
            calculate_recipe(&catalogues, &inventory, "Station Quartermaster T2", "waste").unwrap();

        assert_eq!(result.profession, "Station Quartermaster T2");
    }
}
