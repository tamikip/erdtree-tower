# 无缝入口

运行 `release-preview/Launch-Seamless-Tower.cmd`。它调用游戏目录原有的 `ersc_launcher.exe`，再挂载爬塔 DLL 和资源。直接双击原版 `ersc_launcher.exe` 仍为普通无缝模式。

爬塔存档扩展名为 `ertowercoop`，进度与日志在 `seamless-entry/runtime`。现有密码保留。启动期间使用独立存档并关闭入侵；正常退出游戏后恢复原设置、移除临时引导 DLL。启动窗口需保持开启。已有其他 `dinput8.dll` 时拒绝覆盖。

若启动窗口被强行终止：先退出游戏，使用 `seamless-entry/ersc_settings.before-launch.ini` 恢复游戏目录的 `SeamlessCoop/ersc_settings.ini`；确认游戏目录 `dinput8.dll` 与 `seamless-entry/dinput8.dll` 哈希一致后再删除前者。

2026-10-09 已实测：原无缝入口启动、三个 DLL 同时加载、ModEngine2 资源挂载、爬塔首帧绘制、退出后设置与临时 DLL 恢复。v85 已将骨灰召唤点刷新与可选的使用日志钩子拆开；实机确认刷新钩子已生效，无缝的两个骨灰接口钩子保持原样。用户已确认战斗中成功召唤，日志同时记录召唤物存活；选路、传送、刷怪、领奖也已在无缝单人模式下跑通。多人同步仍未完成；场边服务 NPC 存在反复重建的问题，原因待查。

加载依据：[ModEngine2](https://github.com/soulsmods/ModEngine2) 的入口挂钩和 `MODENGINE_CONFIG`；系统输入 DLL 转发方式参考 [Elden Mod Loader](https://github.com/techiew/EldenRingModLoader)。使用官方 ModEngine2 2.1.0 发布文件。引导仅在本启动器设置的环境标记下启用。

v86：第一幕改回宁姆格福竞技场。已通过现场读取确认旧场边 NPC 坠落死亡；第二、三幕服务 NPC 改用各自入场坐标，避免套用平面竞技场的侧向偏移。91 项测试通过，第二、三幕仍需实机检查位置和交互。
