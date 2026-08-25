# Initial handoff — The Last Carekeeper Utils

Date: 2026-08-25 13:20 (Europe/Madrid)

## Product objective

Build a local utility for *The Last Caretaker* that imports growth resources from a `.sav`, uses only verifiable player-owned items, allows local editing of inventory quantities and item statistics, and calculates human-profession recipes with minimal resource waste. The user supplied `Food.csv`, `Memories.csv`, and `Humans.csv`; all use `;` as the delimiter and contain no duplicate names.

The architecture was subsequently decided: Tauri 2, React and TypeScript for the interface, and Rust for the trusted local core.

## Private reference files

The following samples are local and must not be committed:

- `C:\Users\xisec\Downloads\76561198072091332_0.1.sav`: current sample saved after creating and naming the test chest.
- `C:\Users\xisec\Downloads\76561198072091332_0.sav`: earlier sample without the new chest name.

## Confirmed inventory boundary

The save contains thousands of environmental objects, loot entries, and containers that do not belong to the player. Import must be restricted to:

1. `/Game/Blueprints/BP_FirstPersonCharacter_New.BP_FirstPersonCharacter_New_C`
2. `/Game/Blueprints/Interactives/Containers/BP_Inventory_PlayerBox.BP_Inventory_PlayerBox_C`
3. `/Game/Blueprints/Interactives/Containers/BP_Inventory_PlayerBox_Small.BP_Inventory_PlayerBox_Small_C`

Exclude `BP_Inventory_Container_*`, environmental containers, loose world objects, and every global aggregate.

## Save container format

The save is not encrypted. It is a sequence of custom compressed blocks:

- 8-byte magic: `C1 83 2A 9E 22 22 22 22`
- total header size: 49 bytes
- compressed size: 24-bit little-endian integer at header offsets `+17..+19`
- uncompressed size: 24-bit little-endian integer at offsets `+25..+27`
- zlib/deflate stream beginning at `+49`

The current sample is 5,070,954 compressed bytes and 59,821,774 bytes after decompression. The previous sample is 4,841,958 compressed bytes and expands to 58,750,556 bytes. Concatenated blocks contain a four-byte prefix followed by `GVAS`.

Observed GVAS header:

- SaveGameVersion: 3
- PackageVersion: 522
- UE5 package version: 1018
- Engine: 5.7.4, branch `UE5`
- 93 custom versions
- class: `/Script/Voyage.VoyageSaveGame`
- unversioned properties: `false`

Inventory objects are serialized as `VoyageItemSerialize` structs. The verified scanner identifies the binary `AssetName` property and reads the adjacent `Data.ItemCount` value.

## Named chest evidence

In the current sample, UTF-8 text `PruebaItems001` appears at decompressed offset 24,258,526 (`0x017227DE`). It belongs to the same `BP_Inventory_PlayerBox` actor record spanning approximately `0x0171BE3D` to `0x017294B0`.

The custom label is a `Name` `TextProperty` following `Color`. The actor's component data and `Items` map follow it. This demonstrates that the label and contents can be associated within one player-box record. The earlier sample does not contain this label because it predates the relevant manual save.

## Verified current-sample inventory

The character record spans decompressed offsets 7,370,022 through 7,469,174 and contains exactly four growth stacks:

- `DA_Food_High-FatEnergy`: 1
- `DA_Food_Mind_Surge`: 1
- `DA_Food_Nutri-Core`: 1
- `DA_Food_PhysiqueFuel`: 1

No memories were found in the backpack.

After exact mirror deduplication, `PruebaItems001` contains 26 growth stacks:

```text
DA_Food_Nutri-Core                 9
DA_Food_High-FatEnergy             4
DA_Food_PhysiqueFuel               4
DA_Food_Mind_Surge                 5
DA_Memory_Drawings_Maps            2
DA_Memory_Drawings_Notes           1
DA_Memory_Books_Programming        1
DA_Memory_SmallTree2               2
DA_Memory_Books_FirstAid           2
DA_Memory_Books_Meditation         1
DA_Memory_BasketBall               4
DA_Memory_BasketBall_Blue          2
DA_Memory_Guitar                   3
DA_Memory_Toy                      1
DA_Memory_Drawings_Letters         2
DA_Memory_Stopwatch                2
DA_Memory_Drawings_Cards           2
DA_Memory_Drawings_Kids            2
DA_Memory_Books_Tommy              3
DA_Memory_Drawings_MSurvival       1
DA_Memory_Crayon                   2
DA_Memory_Camera                   5
DA_Memory_Drawings_Blueprints      2
DA_Memory_Drawings_Logs            2
DA_Memory_Books_Encyclopedia       2
DA_Memory_Drawings_Music           1
```

The current save has 23 player-box records; most contain no growth resources.

## Mirrored chest serialization

The game serializes the chest content sequence twice, probably once for the actor and once for a component. Never divide quantities blindly. Collapse a sequence only when it has even length and its ordered second half matches the ordered first half exactly in both asset name and quantity. Preserve a non-matching sequence and emit a diagnostic.

## Canonical backpack serialization

The save captured on 2026-08-25 after moving memories into the backpack decompresses to 59,843,641 bytes. Its character actor spans offsets 7,370,022 through 7,479,116. Eleven memory stacks occur twice with identical names, order and quantities: first after the localized `Equipment` section at `0x70A45E`, then after the localized `Backpack` section at `0x70D2DF`. Four food stacks occur only after those repeated blocks.

`DA_Memory_Drawings_Notes` occurs at `0x70ABD3` and `0x70DE42`, with quantity 1 in both serialized views. It is one owned item, not two stacks. Character inventory extraction must therefore start at the canonical localized `Backpack` section when it is present. The section is accepted only when it is tied to `ST_Inventory` and followed by an `Items` serialization marker. The full-character scan remains a diagnostic compatibility fallback; chest mirror handling remains the stricter exact-whole-sequence rule above.

## Asset aliases

Verified aliases include the four foods and the unambiguous memory names recorded in `data/asset-mappings.json`. `DA_Memory_Drawings_Notes` is verified as `Travel Journal` by an in-game move test and the installed DataAsset. The asset references the `Memory_Drawing_Notes_Name` localization key, its textures are named `T_Memo_Travelnote_*`, and its only human-property reference is `Adaptability` with a value of 10. The technical asset name and localized display name are intentionally different layers.

The following remain deliberately unresolved:

- `DA_Memory_BasketBall_Blue`: not proven equivalent to `Basketball`
- `DA_Memory_Books_Tommy`: `Tommy` and `Where's Tommy` both exist
- `DA_Memory_Drawings_Diagrams`: previously guessed as `Assembly Instructions`

Unknown or ambiguous assets must remain visible and manually assignable rather than silently guessed.

## Installed asset value extraction

The Windows installation inspected on 2026-08-25 is under `F:\SteamLibrary\steamapps\common\Voyage` (Steam app 1783560, build 23962331). Read-only extraction with `retoc` converted 70 `DA_Memory_*` and 15 `DA_Food_*` assets from the IoStore containers; `repak` unpacked the converted copies under the ignored `src-tauri/target/asset-inspection` directory. No installed file was modified.

The converted exports encode each human-property import, followed by its floating-point contribution. A parser built against `retoc`'s legacy package reader associated package indices with property names. It reproduced every checked CSV vector exactly: for example, `DA_Food_High-FatEnergy` is Weight 8 plus Height, Intellect, Life Expectancy and Strength 1; `DA_Memory_Drawings_Notes` is Adaptability 10. The extraction also found currently uncatalogued technical assets such as `DA_Food_Digestive_Overdrive` (Adaptability 5), `DA_Food_Genesis_Prime` (50/50/40/10/10 physical values) and multiple late-game memories.

The next investigation step is already staged locally: converted copies of `ST_Memories` and `ST_Food` exist in `src-tauri/target/asset-inspection`. Parse those string tables to cross-reference localized display names before adding further aliases; do not infer mappings from similar technical names alone.

## Verified analysis tools

`code/analysis` contains the original Node investigation scripts. `report-items.mjs`, `find-text.mjs`, and `analyze-player-boxes.mjs` were executed against the local samples and produced the findings above. The older `code/prototype` directory is reference-only and was never validated as a complete application.

The original `analyze-player-boxes.mjs` display field named `actorName` was simplified and printed the literal property name; do not rely on that display value as an actor identifier.

## Optimizer evidence and constraints

Food affects the five physical attributes and memories affect the ten mental attributes. These groups can be optimized separately and combined. The bounded state uses real inventory quantities and caps progress at profession requirements while retaining actual totals for excess scoring.

The current objectives are minimum excess then item count, or minimum item count then excess. An infeasible result must report deficits. Planning changes never write to the `.sav`.

The public Root-DE calculator was reviewed as comparative research. Its repository is CC BY-NC-ND 4.0; no source code was copied. Production parser and optimizer code in this repository is an independent implementation based on local format evidence and user-supplied CSV data.
