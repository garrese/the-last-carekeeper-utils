# The Last Carekeeper Utils

A local, portable companion application for *The Last Caretaker*. It reads growth items from a save, combines the character backpack with explicitly selected player chests, and calculates resource-efficient human growth recipes.

## Safety model

- The selected `.sav` is opened for reading only. The application never modifies, renames, replaces, or deletes it.
- A save is accepted only when its size and modification time remain stable during the read. If the game writes during the operation, the snapshot is discarded and retried.
- Only the character backpack and exact `BP_Inventory_PlayerBox` / `_Small` actors are eligible. Environmental containers, loose objects, and global-world totals are excluded.
- Chest content is deduplicated only when the two serialized ordered halves match exactly.
- All processing is local. There are no accounts, telemetry, uploads, or runtime internet requirements.

## Portable layout

The release executable uses its own folder as the application workspace:

```text
The Last Carekeeper Utils/
  the-last-carekeeper-utils.exe
  data/       editable CSV catalogues and asset mappings
  config/     persisted save path and chest selection
  backups/    previous versions of edited/imported data
```

Keep the entire folder together. The application creates missing `data`, `config`, and `backups` content on first launch.

## Development

Requirements: Node.js 20+, pnpm, Rust stable, and the Windows prerequisites for Tauri 2.

```powershell
pnpm install
pnpm tauri dev
```

Run the checks with:

```powershell
pnpm build
cargo test --manifest-path src-tauri\Cargo.toml
```

Build a clean portable folder with:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package-portable.ps1
```

The output is created under `portable/`. The script refuses to overwrite an existing package.

## Documentation

- [Architecture and safety](doc/architecture-and-safety.md)
- [Human growth mechanics](doc/human-growth-mechanics.md)
- [Evidence-backed initial handoff](doc/20260825-1320-initial-handoff.md)

