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

The save stores inventory asset references and quantities, not the growth-stat bonuses defined by those assets. Food and memory bonuses therefore come from the editable CSV catalogues. Recovering authoritative bonuses for an unknown asset requires inspecting the game's packaged DataAsset content or another verified source; the application must not infer them from the asset name.

## Optimizer boundary

Food and memories contribute to disjoint stat groups, so each group is solved with bounded dynamic programming under actual inventory limits. The user can prioritize minimum total excess or minimum item count; the other criterion breaks ties.

The result reports requirements, totals, deficits, excess, item quantities, and every profession whose thresholds the recipe satisfies. Applying or editing a recipe changes only in-memory planning quantities. It never writes inventory back to the game save.
