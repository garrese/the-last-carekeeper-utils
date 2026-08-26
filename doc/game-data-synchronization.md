# Game data synchronization

## Purpose

The synchronizer compares the editable portable catalogues with the DataAssets in the locally installed game. It never writes to the game installation or to a save. Normal application use remains fully offline.

The Game data screen exposes `Sync from game` for Food, Memories, Humans, and Mappings. A scan reads all four sections once and opens a review dialog. Changes can be selected individually, by the conservative safe defaults for one section, or cleared before applying.

## Discovery and extraction

On Windows, the application looks for Steam app `1783560` in known Steam libraries and remembers the selected Voyage folder. A directory picker remains available when discovery fails or another installation should be used.

The Rust core opens the installed Unreal IoStore containers with a pinned `retoc` revision. Oodle streams are decompressed by the MIT-licensed pure-Rust `oozextract` crate. The compatibility layer deliberately rejects compression, which keeps this code path read-only by construction.

The scan extracts only the required packages into memory:

- `DA_Food_*` and `DA_Memory_*` item DataAssets;
- profession DataAssets;
- `ST_Food` and `ST_Memories` localization string tables;
- referenced item icon textures.

Converted package imports associate each serialized value with its exact human-property DataAsset. Localization keys provide installed English display names. Technical names are not humanized and accepted as aliases when an authoritative localization entry exists.

Supported inline `PF_B8G8R8A8` icon textures are converted to 64×64 PNG data URLs for the review only. No extracted package or icon is written to disk. Unsupported texture formats degrade to the icon asset path and a warning.

## Comparison policy

Existing manually verified mappings always win over installed display names. This preserves evidence-backed aliases such as `DA_Memory_Drawings_Notes` to `Travel Journal`.

Changes use these states:

- `added`: an authoritative installed entry or mapping is absent locally;
- `changed`: extracted values differ from the corresponding local row;
- `suggested`: a unique full stat vector suggests an alias, but it is not selected automatically;
- `unsupported`, `blocked`, or `conflict`: the result cannot be applied safely with the current schema;
- `missing`: a local entry was not observed in the scan. Missing entries are diagnostic-only and are never deleted.

The item availability columns (`TotalAvailability` and `WorldCount`) are not present in the inspected item DataAssets. Sync therefore preserves existing values and leaves them empty for a new entry.

Profession DataAssets contain profession-specific overrides. The installed catalogue verifies a shared physical baseline of Weight 20, Height 30, and Life Expectancy 10; an explicit profession value replaces that baseline. Each regular category contains four assets whose strictly increasing total requirements establish T1 through T4. This reconstructs all 40 supported profession rows from an empty `Humans.csv`, and the generated rows match the known catalogue field-for-field. `StarChild` remains blocked because its `StarChild` property is outside the calculator's current stat model.

Some installed foods use memory-style properties, such as Adaptability, which the current Food schema cannot represent. These assets remain visible as unsupported instead of being coerced into the wrong columns.

Header-only Food, Memories, and Humans catalogues are valid so synchronization can bootstrap them from zero rows. Installed assets with the same localized display name and identical values share one catalogue row. If their values differ, the production asset is preferred over an asset stored in a `Notused`, `Unused`, or development folder; the latter is shown as a non-applicable conflict. Multiple incompatible production assets remain conflicts instead of creating duplicate names.

## Applying changes

Apply reloads the current portable files and verifies that every selected row still equals the value seen during the scan. A stale review is rejected and must be scanned again.

Progress and apply failures remain visible inside the review dialog. The dialog stays open after an error so the selection and diagnostic context are not lost.

All resulting CSV documents and mappings are validated together before any write starts. The existing portable atomic-write path creates timestamped backups before replacing `Food.csv`, `Memories.csv`, `Humans.csv`, or `asset-mappings.json`. The source game files and saves are never targets of this operation.

## Verification

Unit tests cover localization table parsing, both observed compact property encodings, verified-mapping precedence, missing-entry policy, and BGRA8 thumbnail conversion. An ignored integration test runs against an installed game when available and asserts extraction coverage, nonzero regressions, and decoded icon availability.

