# Third-party notices

Co-op guard-room, input recovery, arena attendant, reconnect and cursor-hook fixes were adapted from the user-provided HansFix3 patch set (based on upstream 832688a). Its package identifies Hans as author and states GPL-3.0-or-later with this project's additional terms. Integration changes and provenance are recorded in `reference/HansFix3-integration.md`; the supplied DLL and launcher configuration were not imported.

Erdtree Tower's own code and data are licensed under GPL-3.0-or-later with additional terms; see `LICENSE` and `ADDITIONAL-TERMS.md`. The third-party components below keep their own licenses.

The preview DLL statically links pinned revisions of fromsoftware-rs (MIT option) and hudhook (MIT).
hudhook is vendored under native/vendor/hudhook from revision 72e1cc062c35df847b703e5507b82eb21d80d82c with the DX12 queue-discovery changes described in its TOWER_PATCH.md.
Earlier private builds included spells derived from RNG Lands - Ruby (Nexus Mods Elden Ring mod 3229). They are removed from this public version; `scene-assets/retired-spell-rows.txt` only lists the removed parameter IDs for stripping and save compatibility.
The optional randomized talisman integration derives effect parameters from the user's RNG Lands - Ruby 0.7.2 package (https://www.nexusmods.com/eldenring/mods/3229). Its provenance and excluded effects are recorded in scene-assets/talisman-build.json. These third-party parameter data retain their original rights and are not relicensed under this project's GPL license. The original archive and its spell item tables are not bundled.
Their notices are included in `licenses/fromsoftware-rs-MIT.txt` and `licenses/hudhook-MIT.txt`.
Other Rust dependencies are recorded in `native/Cargo.lock`; license notices are included in `licenses/rust-dependencies.txt`.

The native character creator layout bridge follows the MIT-licensed ER_Summon example.
Its notice is included in `licenses/ER_Summon-MIT.txt`.
Reference: https://github.com/KamiyamaShiki0704/ER_Summon/tree/20d89fa75b723398d474783cf2b1ecfe7fbf348f

The loader binaries are the unmodified official Mod Engine 3 v0.13.0 Windows distribution.
Its bundled MIT and Apache notices remain in `loader/LICENSE-MIT` and `loader/LICENSE-APACHE`.
Source: https://github.com/garyttierney/me3/releases/tag/v0.13.0

The launcher (`ErdtreeTowerLauncher.exe`, `launcher/`) uses the hyp-launcher-ui page shell (MIT, `licenses/hyp-launcher-ui-MIT.txt`), the Microsoft Edge WebView2 SDK 1.0.2903.40 (BSD-style license and notices in `licenses/WebView2-SDK-LICENSE.txt` and `licenses/WebView2-SDK-NOTICE.txt`), and System.Text.Json 8.0.5 with its .NET support libraries (MIT, `licenses/dotnet-libraries-MIT.txt`, notices in `licenses/dotnet-libraries-THIRD-PARTY-NOTICES.txt`). The WebView2 runtime itself is not bundled; it is part of Windows 11 and installed separately on Windows 10.

The scene-assets folder contains modified base-game regulation, map placement, event, talk-binder and localized text data. Models, textures, animations, game executables and player save files are not bundled; those resources are resolved from the user's installed game.
The development-only scene builder uses SoulsFormats and UXM source checkouts; those tools and their extracted source-map cache are not included in the playable ZIP.
ESD command IDs and bytecode format were checked against the esdlang documentation/source and the user's installed merchant ESD. The preview does not link or bundle esdlang.
Reference: https://github.com/thefifthmatt/ESDLang

Debug item search uses rust-pinyin 0.11.0 (MIT).
Source: https://github.com/mozillazg/rust-pinyin

Copyright (c) 2016 mozillazg

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
## YAFSML

The optional YAFSML 0.11.0 loader by soarqin is bundled in loader/yafsml/ with its LICENSE and THIRD_PARTY_NOTICES.md.
Source: https://github.com/soarqin/YAFSML

## ERNativeUI

The native System settings tab uses ERNativeUI 1.3.0 by Flammrock (MIT).
Source: https://github.com/Flammrock/ERNativeUI/releases/tag/ERNativeUI-v1.3.0
The unmodified host, localization files, LICENSE.txt and THIRD_PARTY_NOTICES.txt are shipped in native-ui/.
The tower configuration provider is built from native/native-settings/; no example mods are shipped.

## BC7 icon decoder

Slim packages include Tower.IconCodec.dll, built from bcdec 0.985 by Sergii Kudlai, commit 80859ed3b7afb1c527a2a99d70c61457bea72d0c.
Source: https://github.com/iOrange/bcdec. Used under the MIT license; see licenses/bcdec.txt.

## Seamless Co-op

Seamless Co-op 2.0.2 by Yui: https://www.nexusmods.com/eldenring/mods/510
Used as a separate runtime component. It is not covered by this project's license.
See `SeamlessCoop/README.md` for provenance and distribution status.

## Markdown-it

Launcher release notes use markdown-it 14.1.0 (MIT), bundled locally.
Source: https://github.com/markdown-it/markdown-it/tree/14.1.0
License: launcher/ui/js/vendor/LICENSE-markdown-it.txt.

## Font Awesome brand icons

QQ and Bilibili icons from Font Awesome Free 6.7.2 by Fonticons, Inc.
Source: https://github.com/FortAwesome/Font-Awesome/tree/6.7.2
Icons are licensed under CC BY 4.0; see licenses/font-awesome.txt.
