# Changelog

All notable user-facing changes to The Last Carekeeper Utils are recorded here.

The project follows a lightweight Keep a Changelog format. Entries remain under
`Unreleased` until the corresponding portable version is prepared. Portable
versions retain the user's four editable catalogue files from the preceding
portable release; their contents are not represented by source-control history.

## Unreleased

### Documentation

- Established this changelog as the running record for functionality, safety,
  compatibility, workflow, and portable-release changes.

## 0.1.6 - 2026-08-26

### Fixed

- Allow header-only Food, Memories, and Humans catalogues, so a user can start
  with empty data and populate it through game synchronization.
- Reconstruct and import the 40 supported profession rows from installed game
  assets when the local Humans catalogue is empty.
- Keep the unsupported `Star Child` profession visible instead of importing it
  with an incomplete stat model.
- Reject duplicate localized item names safely, preferring a production asset
  over a matching unused/development copy and reporting unresolved conflicts.
- Keep synchronization errors and progress in the review dialog, preserving
  the selected changes for correction or retry.

### Changed

- Documented empty-catalogue bootstrap rules and profession-tier inference for
  the installed-game synchronizer.

## 0.1.5 - 2026-08-26

### Added

- `Sync from game` reads locally installed Unreal assets without modifying the
  game or a save, then reviews proposed Food, Memories, Humans, and Mapping
  changes before applying them.
- Per-change and bulk review controls, conservative default selection, and
  item icon previews for supported extracted textures.
- Offline extraction of localized item names, growth values, profession data,
  verified asset aliases, and 64-pixel previews where texture support permits.

### Changed

- Development commands now invoke pnpm through Corepack, avoiding a separate
  globally installed pnpm dependency.
- Portable packaging verifies that the editable data migrated from the previous
  portable version is byte-for-byte identical.

## 0.1.4 - 2026-08-25

### Fixed

- Read the canonical player backpack inventory rather than a mirrored or
  incidental serialized representation.
- Verified `DA_Memory_Drawings_Notes` as `Travel Journal` and recorded the
  mapping used by the inventory and catalogue views.

### Changed

- Portable releases carry `Food.csv`, `Memories.csv`, `Humans.csv`, and
  `asset-mappings.json` forward from the highest previous portable version,
  leaving that previous release intact.

## 0.1.3 - 2026-08-25

### Added

- Inventory diagnostics for unresolved game asset references, including their
  observed counts and enough context to create a manual mapping safely.

### Changed

- Clarified that saves provide item references and quantities, while growth
  bonuses are supplied by the editable local catalogues.

## 0.1.2 - 2026-08-25

### Added

- First portable Tauri application with a React/TypeScript interface and Rust
  domain core.
- Read-only stable-snapshot save loading, player-backpack and selected
  player-chest extraction, editable semicolon-delimited catalogues, and human
  growth recipe optimization.
- Local import/export, backups before catalogue writes, persisted save/chest
  selection, and a Windows-first portable layout.

## Earlier development

### Added

- Initial architecture, safety boundaries, analysis scripts, and evidence
  documentation for parsing The Last Caretaker saves locally.

