# Third-party notices

Erdtree Tower's own code and data are licensed under GPL-3.0-or-later with additional terms; see `LICENSE` and `ADDITIONAL-TERMS.md`. The third-party components below keep their own licenses.

The preview DLL statically links pinned revisions of fromsoftware-rs (MIT option) and hudhook (MIT).
hudhook is vendored under native/vendor/hudhook from revision 72e1cc062c35df847b703e5507b82eb21d80d82c with the DX12 queue-discovery changes described in its TOWER_PATCH.md.
Earlier private builds included spells derived from RNG Lands - Ruby (Nexus Mods Elden Ring mod 3229). They are removed from this public version; `scene-assets/retired-spell-rows.txt` only lists the removed parameter IDs for stripping and save compatibility.
Their notices are included in `licenses/fromsoftware-rs-MIT.txt` and `licenses/hudhook-MIT.txt`.
Other Rust dependencies are recorded in `native/Cargo.lock`; license notices are included in `licenses/rust-dependencies.txt`.

The native character creator layout bridge follows the MIT-licensed ER_Summon example.
Its notice is included in `licenses/ER_Summon-MIT.txt`.
Reference: https://github.com/KamiyamaShiki0704/ER_Summon/tree/20d89fa75b723398d474783cf2b1ecfe7fbf348f

The loader binaries are the unmodified official Mod Engine 3 v0.13.0 Windows distribution.
Its bundled MIT and Apache notices remain in `loader/LICENSE-MIT` and `loader/LICENSE-APACHE`.
Source: https://github.com/garyttierney/me3/releases/tag/v0.13.0

The scene-assets folder contains modified base-game regulation, map placement, event, talk-binder and localized text data. Models, textures, animations, game executables and player save files are not bundled; those resources are resolved from the user's installed game.
The development-only scene builder uses SoulsFormats and UXM source checkouts; those tools and their extracted source-map cache are not included in the playable ZIP.
ESD command IDs and bytecode format were checked against the esdlang documentation/source and the user's installed merchant ESD. The preview does not link or bundle esdlang.
Reference: https://github.com/thefifthmatt/ESDLang
