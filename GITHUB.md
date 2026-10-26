# 仓库与运行包

此仓库保存黄金树爬塔 Mod 的源码、测试和场景资源，包含随机装备词条、Boss 词条与爬塔玩法。可运行包发布在本仓库的 GitHub Release 中。

下载 Release ZIP 后完整解压，运行 `Launch-Preview.cmd`。游戏目录不同于启动脚本默认值时，通过 `Launch-Preview.ps1 -GameDir <游戏Game目录>` 启动。游戏版本检查与现有启动器一致。新词条、新法术请新开旅程体验。

仓库不保存个人存档、会话文件、日志、编译缓存和历史测试包。Release 包也不包含这些文件或旧版 DLL。

Rust 源码与所需 vendored 代码在 `src/`、`native/`，可通过对应的 Cargo.toml 构建。`Build.ps1` 和 `scene-builder/SceneBuilder.csproj` 沿用原工作区布局；完整资源重建仍需原工作区中的 SoulsFormats、UXM、参数定义与游戏文件，仓库未声称包含这些外部依赖。

法术接入的自动验证记录随资源保存。实际施放和原包缺失视觉引用的限制见 README；第三方来源见 THIRD_PARTY_NOTICES.md。

## 协议

作者编写的代码与数据以 GPL-3.0-or-later 发布，并附加保留署名、标明修改版、不授予名称使用权三条附加条款，详见 [LICENSE](LICENSE) 与 [ADDITIONAL-TERMS.md](ADDITIONAL-TERMS.md)。游戏数据、其他 mod 的派生内容和第三方代码不在此协议范围内。
