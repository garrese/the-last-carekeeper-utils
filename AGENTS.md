# The Last Carekeeper Utils

## Purpose

Build a local, read-only companion application for *The Last Caretaker*. It imports growth resources from a player's `.sav`, lets the user adjust inventory data and item statistics locally, and calculates recipes for human professions while minimizing waste.

Read `doc/20260825-1320-initial-handoff.md` before changing save parsing, inventory ownership rules, asset aliases, or optimization behavior. It is the evidence-backed project handoff.

## Product constraints

- This is a downloadable desktop application, not a hosted website and not a local web server.
- Target Windows first. Keep the design portable to macOS/Linux where Tauri supports it.
- The `.sav` is strictly read-only. Never modify, replace, rename, or delete it.
- Process all data locally. No account, telemetry, upload, or internet dependency is required for normal use.
- Import only the character backpack and player-owned `BP_Inventory_PlayerBox` / `_Small` actors.
- Never expose a global-world inventory mode or count environmental containers and loose world objects.
- Deduplicate mirrored chest contents only when both ordered halves match exactly. Otherwise preserve the data and surface a diagnostic.
- Unknown or ambiguous asset aliases must remain visible and manually assignable; do not guess silently.
- CSV files use `;` as the delimiter and remain the editable source of game data.

## Architecture decision

Use Tauri 2 as a native desktop shell:

- React + TypeScript + Vite for the UI. Do not use Next.js, SSR, or a runtime web server.
- Rust for local filesystem integration, zlib decompression, GVAS parsing, inventory extraction, validation, and the optimizer.
- Send compact typed results across Tauri commands; do not transfer the full decompressed save to the webview.
- Keep business logic independent of Tauri APIs so the parser and optimizer can be unit-tested as ordinary Rust libraries.
- Persist only application settings and user overrides beside the portable executable: selected save path, selected chest labels, aliases, statistics, and UI preferences.
- Refresh on explicit user action and on application startup. Read a stable snapshot only after the save metadata remains unchanged; parsing failures must leave the last valid result visible and report the error.
- Scope filesystem capabilities to the configured save location and the app data directory.

Expected high-level modules after scaffolding:

```text
src/                         React UI and typed Tauri client
src-tauri/src/domain/        Save parser, inventory rules, optimizer
src-tauri/src/commands/      Thin Tauri command adapters
src-tauri/tests/             Integration tests using save fixtures
data/                        Shipped CSV catalogues
code/analysis/               Existing verified Node investigation scripts
code/prototype/              Reference-only, unvalidated web prototype
```

Do not treat `code/prototype` as production code. Port behavior deliberately and validate it against the verified Node analysis and fixtures.

## Verification priorities

Before UI work, turn the verified analysis into automated tests covering both sample saves described in the handoff. At minimum assert:

- block decompression and GVAS header values;
- the exact four backpack food stacks in the current sample;
- detection of the `PruebaItems001` player chest and its 26 real growth stacks;
- exact mirrored-half deduplication without halving arbitrary data;
- exclusion of non-player containers and global-world objects;
- explicit reporting of unresolved aliases;
- optimizer feasibility, objective tie-breaking, inventory limits, excess, and deficits.

Prefer small unit fixtures derived from known byte ranges where practical. Full `.sav` fixtures are local/private and must not be committed unless the user explicitly approves it.

## Existing diagnostic commands

The current scripts require Node.js and a local sample path:

```powershell
node code\analysis\report-items.mjs '<path-to-save.sav>'
node code\analysis\find-text.mjs '<path-to-save.sav>' 'PruebaItems001'
node code\analysis\analyze-player-boxes.mjs '<path-to-save.sav>'
```

## Change discipline

- Preserve evidence and document new format findings with offsets, class paths, and fixture expectations.
- Separate parser/domain changes from UI presentation changes.
- Avoid broad filesystem permissions and avoid shell execution from the webview.
- Run focused tests for the changed layer, then the complete parser/optimizer suite before packaging.
