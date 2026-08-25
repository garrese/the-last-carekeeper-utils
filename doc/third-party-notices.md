# Third-party notices

The game-data synchronizer reads installed Unreal Engine container files with the following open-source projects:

- `retoc` 0.1.5, pinned to commit `885a8dae740cb1ce1e41ff2e74f67f9f0c118237`, MIT license.
- `oozextract` 0.5.5, MIT license. It provides pure-Rust decompression for supported Oodle streams; the application does not download or redistribute an Oodle DLL.
- `jmap` 0.1.0, pinned to commit `3ba766e3f2b3035f8c4334ed9598e41122823746`, MIT license. The minimal library crate is vendored because the upstream Git branch has moved to an incompatible version.
- `ser-hex` 0.1.0, pinned to commit `3d890bb069929e9c9c4c817c433f9b8753c264ed`, MIT OR Apache-2.0. The minimal library crate is vendored because the upstream Git branch is not an immutable package source.

The synchronizer is strictly read-only with respect to the game installation. The local `oodle_loader` compatibility crate intentionally rejects compression requests.

