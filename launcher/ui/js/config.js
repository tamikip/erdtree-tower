/* ============================================================
   全部可自定义内容都在这个文件里。
   结构与命名对齐官方前端（左侧 .hyp-side-bar / 右侧 .hyp-game-social-media /
   页面 .app-container），几何数值见 css/tokens.css。
   ============================================================ */

/* ============================================================
   黄金树爬塔启动器的界面内容。界面外壳来自 hyp-launcher-ui（MIT，见 ../LICENSE-hyp-launcher-ui.txt）。
   真实的版本、下载进度、更新状态由宿主程序（launcher/Tower.Launcher）通过 bridge 推送。
   ============================================================ */

/* 无缝联机多人缩放可选的百分比 */
var SCALE_CHOICES = ['0', '5', '10', '15', '20', '25', '30', '35', '40', '45', '50', '60', '70', '80', '90', '100', '120', '150', '200'];

/* 爬塔按键可选的键（与 src/options.rs 的 KEYS 对应的常用子集） */
var KEY_CHOICES = ['Home', 'End', 'Insert', 'Delete', 'PageUp', 'PageDown', 'Pause', 'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12', 'J', 'K', 'L', 'O', 'P', 'U', 'I', 'M', 'N', 'B', 'C', 'V', 'X', 'Z', 'Num0', 'Num1', 'Num2', 'Num3', 'Num4', 'Num5', 'Num6', 'Num7', 'Num8', 'Num9'];

window.HOYO_CONFIG = {
  app: {
    name: '黄金树爬塔',
    primaryGame: 'tower',
  },

  sidebar: {
    logo: 'assets/icons/tower-icon.png',
    games: [
      { key: 'tower', name: '黄金树爬塔', icon: 'assets/icons/tower-icon.png' },
    ],
  },

  /* 右侧栏：网址用系统浏览器打开；action 交给宿主 */
  social: [
    { icon: 'social-community', label: 'QQ 交流群', panel:
      '<p>黄金树爬塔玩家交流群</p>' +
      '<p style="user-select:text;font-size:20px;color:#fff">1 群：1127474860</p>' +
      '<p style="user-select:text;font-size:20px;color:#fff">2 群：996356584</p>' +
      '<p>选中群号后按 Ctrl+C 复制，在 QQ 中搜索群号加入。</p>' },
    { icon: 'social-home', label: '项目主页', href: 'https://github.com/tamikip/erdtree-tower' },
    { icon: 'social-guide', label: '安装教程', action: 'openGuide' },
    { icon: 'social-feedback', label: '问题反馈', href: 'https://github.com/tamikip/erdtree-tower/issues' },
    { icon: 'social-history', label: '更新日志与历史', action: 'releaseHistory' },
    { icon: 'social-folder', label: 'Mod 日志目录', action: 'openModLogs' },
  ],

  windowControls: ['settings', 'minimize', 'close'],

  games: {
    tower: {
      name: '黄金树爬塔',
      background: 'assets/img/tower-bg.webp',
      /* 标题 logo（launcher/make_art.py 生成，2 倍分辨率铺满页面） */
      titleOverlay: 'assets/img/tower-title.png',
      /* 右侧骑士立绘（launcher/make_art.py 生成，透明背景） */
      heroArt: 'assets/img/tower-knight.webp',
      heroSlide: 0,   /* 点立绘弹出的职业介绍：轮播第 1 张（守誓骑士） */
      scrim: true,
      backgrounds: [
        { name: '誓约', art: 'assets/img/tower-bg.webp' },
        { name: '职业', art: 'assets/img/tower-bg-classes.webp' },
      ],
      servers: [],
      pause: false,
      accent: '#d8a848',
      accentInk: '#1a1206',
      chip: null,
      /* 点击轮播图弹出职业介绍（detail 与 native/src/class_ui.rs 的 ROLES 一致，tests/test_launcher_classes.py 检查） */
      carousel: [
        { img: 'assets/img/tower-banner-1.webp', href: 'https://github.com/tamikip/erdtree-tower',
          detail: { name: '守誓骑士', role: '坦克 · 举盾防守', mechanic: ['誓守', '举盾防御时受到的所有伤害降低 25%（含物理）；防御时精力消耗降低 30%'], kit: '长剑、斧枪、鸢形盾', bias: '直剑、大剑、斧枪、中盾与大盾', accent: 'rgb(196, 160, 96)' } },
        { img: 'assets/img/tower-banner-2.webp', href: 'https://github.com/tamikip/erdtree-tower',
          detail: { name: '狂战士', role: '重武器 · 硬抗', mechanic: ['狂血', '受到的削韧降低 30%；生命低于一半时受到的伤害降低 15%；打赢一场战斗回复 8% 生命'], kit: '巨斧、战斧、大皮盾', bias: '斧、大斧、槌、大槌、特大武器', accent: 'rgb(200, 84, 64)' } },
        { img: 'assets/img/tower-banner-3.webp', href: 'https://github.com/tamikip/erdtree-tower',
          detail: { name: '影刃', role: '敏捷 · 致命一击', mechanic: ['影步', '致命一击后 10 秒内对敌人的伤害提高 20%；精力恢复速度提高'], kit: '打刀、长弓、红棘木盾', bias: '刀、匕首、曲剑、弓', accent: 'rgb(150, 172, 190)' } },
        { img: 'assets/img/tower-banner-4.webp', href: 'https://github.com/tamikip/erdtree-tower',
          detail: { name: '星辉术士', role: '远程法术', mechanic: ['星涌', '进入战斗时专注回复一半；魔法专注消耗降低 15%'], kit: '辉石杖、短剑、辉石魔砾、辉石彗星', bias: '法杖与魔法', accent: 'rgb(110, 140, 220)' } },
        { img: 'assets/img/tower-banner-5.webp', href: 'https://github.com/tamikip/erdtree-tower',
          detail: { name: '祷誓者', role: '祷告 · 治疗 · 辅助', mechanic: ['圣誓', '祷告专注消耗降低 15%；打赢一场战斗回复 25% 专注'], kit: '短矛、指头圣印记、治疗、招来火焰', bias: '圣印记、祷告与矛', accent: 'rgb(226, 196, 120)' } },
        { img: 'assets/img/tower-banner-6.webp', href: 'https://github.com/tamikip/erdtree-tower',
          detail: { name: '召唤师', role: '骨灰为核心', mechanic: ['群灵', '战斗胜利时召唤物还活着，这次召唤不消耗骨灰；每页奖励都有骨灰，召唤流余烬必定可出'], kit: '短剑、熨斗形盾、圣印记、离群野狼与骸骨民兵的骨灰', bias: '骨灰、轻武器与祷告', accent: 'rgb(140, 196, 170)' } },
      ],
      /* 公告：宿主检查更新后换成最新版本的更新说明 */
      news: {
        activity: [{ title: '六个职业、Boss 词条与苦难律法', date: '', href: 'https://github.com/tamikip/erdtree-tower' }],
        notice: [{ title: '更新说明见发布页', date: '', href: 'https://github.com/tamikip/erdtree-tower/releases' }],
        info: [{ title: '安装教程与常见问题', date: '', href: 'https://github.com/tamikip/erdtree-tower' }],
      },
      cta: { text: '安装', launchText: '开始游戏', hint: '模组会安装到启动器所在的文件夹' },
      installed: false,
      size: 0,
      patchSize: 0,
      version: '',
      updateTo: null,
      progress: null,
    },
  },

  hub: {
    background: 'assets/img/tower-bg.webp',
    bgDim: 0.3,
    logo: 'assets/icons/tower-icon-192.png',
    description: '艾尔登法环的爬塔 Mod：六个职业、随机词条装备、Boss 词条、誓约与余烬、苦难律法，支持无缝联机。',
    entries: [
      { key: 'tower', name: '黄金树爬塔', card: 'assets/icons/tower-icon-192.png', art: 'assets/img/tower-bg.webp',
        landLogo: 'assets/icons/tower-icon-192.png', page: 'tower' },
    ],
  },

  /* ---------- 多语言 ----------
     key 就是界面文案的查找键；没配到的语言会回退到 zh-CN。
     切换语言由设置里的 lang 项驱动（见 app.js 的 applyLocalSettings）。 */
  locales: {
    'zh-CN': {
      'task.downloading': '正在下载 ',
      'task.updating': '正在更新 ',
      'task.paused': '已暂停',
      'task.retry': '重试',
      'task.install': '安装',
      'task.queuedShort': '排队中',
      'task.queued': '已加入队列，等前面的下载完成',
      'task.preStarted': '已加入预下载，完成后可手动安装',
      'task.preDone': '预下载完成，可以安装了',
      'phase.verifying': '校验中',
      'phase.extracting': '解压中',
      'phase.installing': '安装中',
      'phase.ready': '待安装',
      'menu.launch': '启动游戏',
      'menu.locate': '打开安装目录',
      'menu.verify': '校验文件完整性',
      'menu.uninstall': '卸载',
      'menu.download': '下载',
      'menu.pre': '预下载',
      'menu.launched': '已请求启动',
      'menu.verifying': '开始校验文件…',
      'uninstall.title': '卸载',
      'uninstall.ask': '确定要卸载 ',
      'uninstall.note': '卸载会删除游戏文件，存档默认保留（由宿主决定）。',
      'uninstall.ok': '卸载',
      'uninstall.done': '已卸载',
      'common.cancel': '取消',
      'common.tip': '提示',
      'account.guest': '未登录',
      'account.switch': '切换账号',
      'account.logout': '退出登录',
      'account.settings': '设置',
      'account.quit': '退出启动器',
      'account.quitSent': '已请求退出（需宿主接管）',
      'account.switchSent': '请在宿主里完成账号切换',
      'account.logoutSent': '已请求退出登录',
      'social.opening': '正在用浏览器打开',
      'social.todo': '这里可以接你自己的页面或宿主功能。',
      'social.hint': '在 config 里给这一项加 action 或 panel 字段即可生效。',
      'bg.switched': '已切换背景',
      'server.switched': '已切换到 ',
      'win.needHost': '窗口控制需要宿主 exe（浏览器里不可用）',
    },
    en: {
      'task.downloading': 'Downloading ',
      'task.updating': 'Updating ',
      'task.paused': 'Paused',
      'task.retry': 'Retry',
      'task.install': 'Install',
      'task.queuedShort': 'Queued',
      'task.queued': 'Queued — will start when a slot frees up',
      'task.preStarted': 'Pre-download started; install manually when done',
      'task.preDone': 'Pre-download complete, ready to install',
      'phase.verifying': 'Verifying',
      'phase.extracting': 'Extracting',
      'phase.installing': 'Installing',
      'phase.ready': 'Ready',
      'menu.launch': 'Launch',
      'menu.locate': 'Open folder',
      'menu.verify': 'Verify files',
      'menu.uninstall': 'Uninstall',
      'menu.download': 'Download',
      'menu.pre': 'Pre-download',
      'menu.launched': 'Launch requested',
      'menu.verifying': 'Verifying files…',
      'uninstall.title': 'Uninstall',
      'uninstall.ask': 'Uninstall ',
      'uninstall.note': 'Game files will be removed; saves are kept (host decides).',
      'uninstall.ok': 'Uninstall',
      'uninstall.done': 'Uninstalled',
      'common.cancel': 'Cancel',
      'common.tip': 'Notice',
      'account.guest': 'Not signed in',
      'account.switch': 'Switch account',
      'account.logout': 'Sign out',
      'account.settings': 'Settings',
      'account.quit': 'Quit launcher',
      'account.quitSent': 'Quit requested (host must handle)',
      'account.switchSent': 'Complete sign-in in the host',
      'account.logoutSent': 'Sign-out requested',
      'social.opening': 'Opening in browser: ',
      'social.todo': 'Hook your own page or host feature here.',
      'social.hint': 'Add an action or panel field to this item in config.',
      'bg.switched': 'Background: ',
      'server.switched': 'Switched to ',
      'win.needHost': 'Window controls need the host exe (unavailable in a browser)',
    },
  },

  account: {
    avatar: 'assets/icons/tower-icon.png',
    nick: '褪色者',
    uid: '',
  },

  settings: {
    title: '设置',
    appName: '黄金树爬塔启动器',
    version: '',
    builder: 'Erdtree Tower',
    appIcon: 'assets/icons/tower-icon.png',
    links: [
      { label: '项目主页', href: 'https://github.com/tamikip/erdtree-tower' },
      { label: '第三方开源许可', action: 'openLicenses' },
    ],
    copyright: '黄金树爬塔 · GPL-3.0-or-later。界面外壳基于 hyp-launcher-ui（MIT）。艾尔登法环为其权利人所有。',
    tabs: [
      {
        key: 'game', name: '游戏',
        groups: [
          {
            title: '启动',
            rows: [
              { type: 'seg', key: 'mode', label: '启动模式', value: '单人', options: ['单人', '无缝联机'] },
              { type: 'static', label: '游戏目录', key: 'gameDir', value: '（未设置）' },
              { type: 'buttons', label: '目录', items: [{ text: '选择游戏目录…', action: 'chooseGameDir' }] },
            ],
          },
          /* 下面两组写进模组目录的 tower-options.ini，游戏启动时读取（用 .cmd 启动也生效） */
          {
            title: 'DLC（下次启动游戏生效）',
            rows: [
              { type: 'select', key: 'dlc', label: '黄金树之影 DLC', hint: '只对单人生效；无缝联机时双方都要有 DLC',
                value: '有 DLC', options: ['有 DLC', '没有 DLC', '没有 DLC，保留 DLC 武器（实验）'] },
            ],
          },
          {
            title: '调试（下次启动游戏生效）',
            rows: [
              { type: 'switch', key: 'debug', label: '调试模式', hint: '游戏里按「调试菜单」键打开；联机时调试命令不执行', value: false },
            ],
          },
          {
            title: '按键（下次启动游戏生效）',
            rows: [
              { type: 'select', key: 'key.settle', label: '快速结算', hint: '暂离并结束本局', value: 'Home', options: KEY_CHOICES },
              { type: 'select', key: 'key.overlay', label: '显示/隐藏爬塔界面', value: 'F10', options: KEY_CHOICES },
              { type: 'select', key: 'key.archive', label: '旅程档案', value: 'J', options: KEY_CHOICES },
              { type: 'select', key: 'key.build', label: '构筑面板（地图界面）', value: 'C', options: KEY_CHOICES },
              { type: 'select', key: 'key.debug', label: '调试菜单', value: 'F8', options: KEY_CHOICES },
            ],
          },
        ],
      },
      {
        key: 'plugins', name: '插件',
        groups: [
          {
            title: '额外 DLL 插件（下次启动游戏生效，单人和无缝联机都会加载）',
            rows: [
              { type: 'plugins' },
              { type: 'buttons', label: '',
                items: [{ text: '添加 DLL…', action: 'addPluginDll', primary: true }, { text: '添加插件文件夹…', action: 'addPluginFolder' },
                  { text: '打开插件文件夹', action: 'openPlugins' }] },
            ],
          },
          {
            title: '说明',
            rows: [
              { type: 'static', label: '单个 DLL', value: '连同同名的配置文件、同名文件夹一起复制' },
              { type: 'static', label: '文件夹', value: '整个文件夹复制，顶层每个 DLL 都登记，用不到的关掉' },
              { type: 'static', label: '手动配置', value: 'plugins\\natives.toml（ME3 的 [[natives]] 格式）' },
            ],
          },
        ],
      },
      {
        key: 'coop', name: '联机',
        groups: [
          {
            title: '无缝联机（Seamless Co-op，需自行安装到游戏目录）',
            rows: [
              { type: 'static', label: '版本', key: 'erscInfo', value: '—' },
              { type: 'buttons', label: '',
                items: [{ text: '下载页', action: 'seamlessDownload' }, { text: '打开 SeamlessCoop 文件夹', action: 'seamlessFolder' },
                  { text: '打开配置文件', action: 'seamlessIni' }] },
            ],
          },
          {
            title: '联机设置（写入游戏目录的 ersc_settings.ini，下次启动生效）',
            rows: [
              { type: 'input', key: 'ersc.cooppassword', label: '联机密码', hint: '双方填同一个；最多 64 字，不能含 ; # =', placeholder: '输入后按回车或点别处保存', value: '' },
              { type: 'static', label: '死亡惩罚', value: '关闭（爬塔固定）' },
              { type: 'switch', key: 'ersc.allow_summons', label: '允许召唤骨灰', hint: '关掉后用不了骨灰（召唤师职业依赖骨灰）', value: true },
              { type: 'switch', key: 'ersc.skip_splash_screens', label: '跳过开场动画', value: false },
            ],
          },
          {
            title: '多人缩放（爬塔预设，每多一名玩家增加的百分比）',
            rows: [
              { type: 'select', key: 'ersc.enemy_health_scaling', label: '敌人生命', value: '35', options: SCALE_CHOICES },
              { type: 'select', key: 'ersc.enemy_damage_scaling', label: '敌人伤害', value: '0', options: SCALE_CHOICES },
              { type: 'select', key: 'ersc.enemy_posture_scaling', label: '敌人韧性', value: '15', options: SCALE_CHOICES },
              { type: 'select', key: 'ersc.boss_health_scaling', label: 'Boss 生命', value: '100', options: SCALE_CHOICES },
              { type: 'select', key: 'ersc.boss_damage_scaling', label: 'Boss 伤害', value: '0', options: SCALE_CHOICES },
              { type: 'select', key: 'ersc.boss_posture_scaling', label: 'Boss 韧性', value: '20', options: SCALE_CHOICES },
            ],
          },
          {
            title: '由爬塔的无缝入口决定（在配置文件里改了也不生效）',
            rows: [
              { type: 'static', label: '存档后缀', value: 'ertowercoop（与单人、原版无缝存档分开）' },
              { type: 'static', label: '入侵', value: '关闭' },
              { type: 'static', label: '死亡后观战', value: '开启' },
            ],
          },
        ],
      },
      {
        key: 'update', name: '更新',
        groups: [
          {
            title: '模组文件',
            rows: [
              { type: 'switch', key: 'autoCheck', label: '启动时自动检查更新', value: true },
              { type: 'seg', key: 'route', label: '下载线路', hint: '先走选中的线路；连不上或版本落后时自动换另一条',
                value: '国内镜像', options: ['国内镜像', 'GitHub'] },
              { type: 'static', label: '国内镜像', key: 'mirrorInfo', value: '—' },
              { type: 'static', label: '整合包版本', key: 'version', value: '—' },
              { type: 'static', label: '启动器版本', key: 'launcherVersion', value: '—' },
              { type: 'static', label: '更新状态', key: 'updateStatus', value: '尚未检查' },
              { type: 'buttons', label: '更新',
                items: [{ text: '检查更新', action: 'checkUpdate', primary: true }, { text: '校验并修复文件', action: 'verify' }, { text: '更新日志与历史', action: 'releaseHistory' }] },
              { type: 'buttons', label: '日志',
                items: [{ text: 'Mod 日志目录', action: 'openModLogs' }, { text: '启动器日志目录', action: 'openLogs' }] },
            ],
          },
        ],
      },
      {
        key: 'general', name: '常规',
        groups: [
          {
            title: '界面',
            rows: [
              { type: 'switch', key: 'closeExit', label: '启动游戏后关闭启动器', value: false },
              { type: 'switch', key: 'reduceMotion', label: '减少界面动效', hint: '关闭呼吸、闪烁等循环动画', value: false },
            ],
          },
        ],
      },
      {
        key: 'about', name: '关于',
        groups: [{ rows: [{ type: 'about' }] }],
      },
    ],
  },
};
