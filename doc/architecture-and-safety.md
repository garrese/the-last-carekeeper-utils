# Architecture and safety

## Decision

The application uses Tauri 2 with a React, TypeScript, and Vite interface plus a Rust core. It is a native desktop application: release builds do not start or require a web server.

The webview is limited to presentation and compact typed command results. Rust owns filesystem access, save decompression and parsing, catalogue validation, persistence, and recipe optimization. Domain modules do not depend on Tauri APIs, so their behavior can be tested as ordinary Rust code.

## Portable persistence

In a release build, the executable directory is the portable workspace. User-editable state is stored in `data/`, `config/`, and `backups/` beside the executable. Development builds use the repository root.

CSV catalogues remain the source of truth and use `;` as their delimiter. An interface edit writes the corresponding CSV using a temporary file and creates a timestamped backup first. Import and export are conveniences around those same validated documents.

## Save boundary

The `.sav` path is persisted, but the file itself is outside the application's write boundary. Import follows this sequence:

1. Read file metadata.
2. Wait briefly so an in-progress manual game save can settle.
3. Open and read the file without write access.
4. Read metadata again.
5. Accept the bytes only if size and modification time are unchanged; otherwise retry.
6. Validate block headers, declared sizes, zlib streams, the `GVAS` marker, actor classes, and inventory payloads.

A failed refresh does not mutate the save or catalogue files. The interface reports the error so the user can retry after the game finishes saving.

## Inventory ownership

The parser recognizes only:

- `/Game/Blueprints/BP_FirstPersonCharacter_New.BP_FirstPersonCharacter_New_C`
- `/Game/Blueprints/Interactives/Containers/BP_Inventory_PlayerBox.BP_Inventory_PlayerBox_C`
- `/Game/Blueprints/Interactives/Containers/BP_Inventory_PlayerBox_Small.BP_Inventory_PlayerBox_Small_C`

The backpack is always included. Player boxes are included only when their custom label matches a configured label, case-insensitively. No global inventory or environmental-container mode exists.

Unknown asset names remain visible as unresolved diagnostics. Ambiguous aliases are not guessed. Mirrored chest sequences are collapsed only when both ordered halves are exactly identical; a mismatch is preserved and reported.

### Backpack serialization boundary

The current private save fixture's character actor starts at decompressed offset `0x707526`. Its localized `Backpack` descriptor is at `0x70BF70` and is followed by two tagged `ByteData` properties: descriptor data and live backpack state. The live state's `Items` map spans `0x70C4B3..0x70DD42`; it contains no growth resources in this fixture.

A later `ByteData` property starts at `0x70E0EF` and contains a separate `Items` map at `0x70E131`. Four historical food records of quantity one occur there at `0x71D942`, `0x71DBFC`, `0x71DEB2`, and `0x71E168`. They are not backpack ownership. Backpack extraction is therefore bounded to the declared data range of the live-state `Items` property and must never scan from the `Backpack` label to the end of the character actor. The fixture expectation is an empty growth-resource backpack plus chest quantities 4 High-Fat, 5 Mind Surge, 9 Nutri-Core, and 4 Physique Fuel.

The save stores inventory asset references and quantities, not the growth-stat bonuses defined by those assets. Food and memory bonuses therefore come from the editable CSV catalogues. The optional installed-game synchronizer can recover authoritative bonuses, localization keys, and supported icons from packaged DataAssets in a separate read-only operation. Every difference is reviewed before it updates a local CSV or mapping; it never infers aliases from similar technical names.

## Optimizer boundary

Food and memories contribute to disjoint stat groups, so each group is solved with bounded dynamic programming under actual inventory limits. The user can prioritize minimum total excess or minimum item count; the other criterion breaks ties.

The result reports requirements, totals, deficits, excess, item quantities, and every profession whose thresholds the recipe satisfies. Applying or editing a recipe changes only in-memory planning quantities. It never writes inventory back to the game save.
