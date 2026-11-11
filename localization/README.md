# English localization

Select **English** in the launcher and set Elden Ring itself to English for a fully English game. The launcher option controls the tower UI and themed textures; native item and dialogue FMGs follow the game's language. Changes take effect after restarting the game. Auto mode reads the Steam installation language for textures and ERNativeUI's game-language API for runtime text; select English explicitly if the installation has no Steam language manifest.

Translations are applied only when displaying text. Save keys, item IDs, network messages, and gameplay comparisons retain their original values. Player names are retained. English searches accept localized item, armor, and enemy names alongside Chinese and pinyin.

`official-en.json` is generated from the installed game's English FMGs. Custom catalogs override it. Unknown strings deliberately remain unchanged so coverage audits can identify missing translations.

## Regenerating native text

Use a separate staging directory. `SceneBuilder` arguments are game directory, output directory, input scene-assets directory, mode, and optional mode input:

1. Run `localization-export` to export original language tables and changed Chinese entries.
2. Run `python localization/generate_official_catalog.py <staging>`.
3. Run `python localization/prepare_fmg.py <staging>`.
4. Run `cargo run --release --example localize -- <staging>/fmg-input.json <staging>/fmg-translated.json`.
5. Run `python localization/assemble_fmg.py <staging>`. This rejects untranslated text and changed markup, and fits weapon captions while retaining affix information.
6. Run `english-text <staging>/fmg-english.json` into a second output directory. Copy only its `msg/engus` directory back into `scene-assets/msg/engus` after verification. Do not package the extracted `originals` directory.

For the title and loading tips, pass `scene-builder/title_theme.en.json` to `title_textures.write_dds`, then run `english-theme <texture-directory>`. Copy the verified `menu` output to `scene-assets/locales/en-US/menu`. This replaces only the six theme textures, retaining other UI resources. ME3 and YAFSML load the language package after the base package. Slim distributions reconstruct the English high-quality archive locally and omit unused low-quality overrides.

`generate_native_settings.py` runs during native settings builds and rejects untranslated C++ menu strings. Build and ship the updated tower DLL and native settings DLL together with the English resources and launcher scripts. Do not install just the game DLL.
