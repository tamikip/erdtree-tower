# 黄金树爬塔（Erdtree Tower）协议说明与附加条款

Copyright (C) 2026 tamikip

本项目中由作者编写的部分以 **GNU 通用公共许可证第 3 版或任何更新版本（GPL-3.0-or-later）** 发布，许可证全文见 [LICENSE](LICENSE)。依据 GPL-3.0 第 7 条，作者另附下列附加条款。附加条款与 GPL-3.0 一并适用于本项目及其任何修改版本。

中文与英文条款含义相同，如有出入，以英文为准。

## 一、协议覆盖范围

**适用 GPL-3.0-or-later 的部分**：作者编写的源代码、脚本、构建工具、测试、文档，以及作者编写的规则数据表（如路线、怪物池、奖励池、词条目录等文本表格）。包括 `src/`、`native/src/`、`scene-builder/`、`tests/`、`distribution/`、根目录脚本及 `scene-assets/` 中由作者编写的数据。

**不适用本协议的部分**（保留其原有权利或许可）：

1. **艾尔登法环游戏数据**：`scene-assets/` 中基于游戏文件修改或提取的参数（`regulation.bin`）、地图、事件、对话、文本包、物品图标与游戏原文文本等，其权利属于 FromSoftware 与 Bandai Namco。本项目不授予这些内容的任何权利，使用者须自行拥有正版游戏。
2. **其他 mod 的内容**：本项目早期私有版本曾集成 RNG Lands - Ruby，公开版已全部移除；`scene-assets/retired-spell-rows.txt` 只记录被移除参数的编号。若今后引入其他 mod 的内容，适用其原作者的许可。
3. **第三方代码与工具**：`native/vendor/` 中的 fromsoftware-rs、hudhook，以及 Mod Engine 3、Rust 依赖等，适用各自许可证，见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 与 `licenses/`。

## 二、附加条款（GPL-3.0 第 7 条）

1. **保留署名**（第 7 条 b 项）：分发本项目或其修改版本时，须在 README、发布页或模组页面的说明中保留以下署名，且不得删除源文件中的版权声明：
   > 基于 tamikip 的黄金树爬塔（Erdtree Tower）：https://github.com/tamikip/erdtree-tower

2. **标明修改、不得冒充原版**（第 7 条 c 项）：修改版本须以合理方式标明其为修改版，例如在名称或版本号中注明，并在说明中写明修改内容概要；不得声称或暗示修改版本是原版、官方版本，或已获作者认可。

3. **不授予名称权利**（第 7 条 e 项）：本协议不授予使用“黄金树爬塔”“Erdtree Tower”名称及本项目封面图、标志作为修改版本名称或宣传主体的权利。修改版本须使用不同的名称，可以如实说明“基于黄金树爬塔”。

## 三、对使用者的提示（不属于附加条款）

- 发布编译后的 DLL 或整合包时，须同时提供对应的完整源代码，或按 GPL-3.0 第 6 条提供获取源代码的方式，修改版本同样须以 GPL-3.0 开源。
- 本项目按“原样”提供，不附带任何担保。使用 mod 存在被游戏在线功能封禁的风险，请仅在离线模式下使用。

---

# Erdtree Tower — License Scope and Additional Terms

Copyright (C) 2026 tamikip

The parts of this project written by the author are released under the **GNU General Public License, version 3 or (at your option) any later version (GPL-3.0-or-later)**; see [LICENSE](LICENSE). Under section 7 of the GPL-3.0, the following additional terms apply to this project and to any modified version of it.

## 1. Scope

**Covered by GPL-3.0-or-later:** source code, scripts, build tools, tests, documentation and rule data tables written by the author (route, enemy pool, reward pool and affix catalog tables and similar), including `src/`, `native/src/`, `scene-builder/`, `tests/`, `distribution/`, scripts in the repository root, and author-written data in `scene-assets/`.

**Not covered** (these keep their original rights or licenses):

1. **ELDEN RING game data:** parameters (`regulation.bin`), maps, events, talk scripts, text archives, item icons and original game text in `scene-assets/` that are modified from or extracted from game files belong to FromSoftware and Bandai Namco. No rights to them are granted; users must own the game.
2. **Content from other mods:** earlier private builds included RNG Lands - Ruby; it is entirely removed from this public version, and `scene-assets/retired-spell-rows.txt` only lists the IDs of the removed parameters. Any content from other mods added later keeps its original author's license.
3. **Third-party code and tools:** fromsoftware-rs and hudhook in `native/vendor/`, Mod Engine 3 and the Rust dependencies keep their own licenses; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and `licenses/`.

## 2. Additional terms (GPL-3.0 section 7)

1. **Attribution (section 7(b)):** anyone distributing this project or a modified version must keep the following attribution in its README, release page or mod page description, and must not remove copyright notices from source files:
   > Based on Erdtree Tower (黄金树爬塔) by tamikip: https://github.com/tamikip/erdtree-tower

2. **Marking of modified versions (section 7(c)):** modified versions must be marked in reasonable ways as different from the original, for example in their name or version string, with a summary of the changes in their description. They must not claim or imply to be the original or an official version, or to be endorsed by the author.

3. **No rights to the names (section 7(e)):** this license does not grant rights to use the names "黄金树爬塔" or "Erdtree Tower", or the project's cover art or logos, as the name or main branding of a modified version. Modified versions must use a different name; they may truthfully state that they are "based on Erdtree Tower".

## 3. Notes for users (not additional terms)

- Distributing a compiled DLL or package requires providing the complete corresponding source code, or a way to obtain it as described in section 6 of the GPL-3.0; modified versions must also be released under the GPL-3.0.
- This project is provided "as is" without any warranty. Using mods can lead to bans from the game's online features; use it offline only.
