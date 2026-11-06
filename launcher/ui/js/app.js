/* ============================================================
   应用主逻辑：外壳渲染 + 路由 + 启动页/Hub页 + 交互
   ============================================================ */

(function () {
  'use strict';

  var CFG = window.HOYO_CONFIG;
  var bridge = window.hoyoBridge;
  var UI = window.hoyoUI;

  var state = {
    route: 'launcher', // 'launcher' | 'hub'
    game: CFG.app.primaryGame || (CFG.hub.entries[0] && CFG.hub.entries[0].key),
    tab: 'activity',
    slide: 0,
    playing: true,
    hubIndex: 0, // Hub 页当前选中的卡片序号
    server: 0,   // 当前区服序号
    bgIndex: 0,  // 当前背景序号
    loaded: false, // Hub 首屏是否已过骨架屏
  };

  var carouselTimer = null;

  /* ================= 设置持久化 =================
     存 localStorage（宿主也可以接管：回 settings/loaded 覆盖）。
     这里只管"能本地生效"的那几项，业务性的交给宿主。
     ============================================== */
  var STORE_KEY = 'hoyo-launcher-settings';

  function loadStore() {
    try { return JSON.parse(localStorage.getItem(STORE_KEY) || '{}'); }
    catch (e) { return {}; }
  }

  function saveStore(patch) {
    var o = loadStore();
    Object.keys(patch).forEach(function (k) { o[k] = patch[k]; });
    try { localStorage.setItem(STORE_KEY, JSON.stringify(o)); } catch (e) { /* 忽略 */ }
    return o;
  }

  /* 影响下载行为的运行时参数。
     直接从持久化存储读，而不是从设置页的 setState ——
     否则某套 profile 没配「下载限速」这一行，限速就直接失效。 */
  var runtime = { limitSpeed: 0, maxTask: 3 };

  function loadRuntime() {
    var s = loadStore();
    runtime.limitSpeed = +s.limitSpeed || 0;
    runtime.maxTask = +s.maxTask || 3;
    return runtime;
  }

  /* 把设置里能本地生效的部分应用到界面 */
  function applyLocalSettings(vals) {
    var v = vals || loadStore();

    /* 语言：先把所有字典注册进去，再设当前语言。
       之前只在"设置里切语言"时注册，导致默认启动时 t() 全是裸 key。 */
    if (CFG.locales) {
      Object.keys(CFG.locales).forEach(function (k) {
        UI.setLocale(k, CFG.locales[k]);
      });
    }
    var want = v.lang || 'zh-CN';
    UI.setLocale(want, CFG.locales ? CFG.locales[want] : null);

    /* 动效：系统偏好 + 设置强开 */
    var sysReduced = window.matchMedia &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    UI.applyMotion(sysReduced || !!v.reduceMotion);

    /* 字号缩放 */
    if (v.fontScale) {
      document.documentElement.style.setProperty(
        '--font-scale', (v.fontScale / 100).toFixed(2));
    }
  }

  /* 键盘可达 / 焦点圈：ui.js 已接管 html.kb 的切换 */
  function initA11y() {
    UI.initTooltips(document);
  }

  /* ================= 窗口尺寸自适应 =================
     设计画布固定 1280x720。视口比它小就整体等比缩小并居中，
     比它大则保持原尺寸（stage 模式下由 stage 规则居中）。
     WebView2 的 CSS 视口随系统 DPI 变化，不能假定与窗口像素相同。
     ================================================= */
  function fitCanvas() {
    var app = document.getElementById('app');
    if (!app) return;
    var stage = document.documentElement.classList.contains('stage');
    if (stage) { app.style.transform = ''; return; }
    var vw = window.innerWidth;
    var vh = window.innerHeight;
    var s = Math.min(1, vw / 1280, vh / 720);
    app.style.left = Math.max(0, (vw - 1280 * s) / 2) + 'px';
    app.style.top = Math.max(0, (vh - 720 * s) / 2) + 'px';
    app.style.transform = 'scale(' + s + ')';
  }

  /* ---------------- 工具 ---------------- */
  function el(tag, cls, html) {
    var n = document.createElement(tag);
    if (cls) n.className = cls;
    if (html != null) n.innerHTML = html;
    return n;
  }

  function esc(s) {
    return String(s == null ? '' : s).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
  }

  function gameOf(key) {
    return CFG.games[key] || null;
  }

  /* 文案：优先走 i18n 字典，没有再回退到传入的默认值 */
  function T(key, fb) {
    return UI ? UI.t(key, fb) : (fb != null ? fb : key);
  }

  function toast(msg, type, ms) {
    if (UI) UI.toast(msg, { type: type || 'info', duration: ms });
  }

  /* 当前游戏的主题色注入到根变量，切换游戏时整体变色 */
  function applyAccent(game) {
    /* 品牌色：主按钮底色 / 下划线 / 选中描边 / 悬停高亮都由 --brand 驱动。
       优先级：当前游戏页的 accent > config.app.accent（整套配置的默认色）> 默认主色。
       Hub 页没有"当前游戏"，就落到第二级，所以换主题只要在 app 里写一次。 */
    var A = (CFG && CFG.app) || {};
    var accent = (game && game.accent) || A.accent || '#ffdb29';
    var ink = (game && game.accentInk) || A.accentInk || '#16171a';
    var root = document.documentElement.style;
    root.setProperty('--brand', accent);
    root.setProperty('--brand-ink', ink);
    root.setProperty('--c-yellow-soft', accent);
  }

  /* ---------------- 外壳：窗口控制 ---------------- */
  var CONTROL_SVG = {
    settings: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" ' +
      'stroke-width="1.7" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/>' +
      '<path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33A1.65 1.65 0 0 0 14 20.83V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15 1.65 1.65 0 0 0 3.17 14H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68 1.65 1.65 0 0 0 10 3.17V3a2 2 0 0 1 4 0v.09A1.65 1.65 0 0 0 15 4.6a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.32 9 1.65 1.65 0 0 0 20.83 10H21a2 2 0 0 1 0 4h-.09A1.65 1.65 0 0 0 19.4 15Z"/></svg>',
    minimize: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" ' +
      'stroke-width="2.4" stroke-linecap="round"><path d="M4 12h16"/></svg>',
    maximize: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" ' +
      'stroke-width="2"><rect x="5" y="5" width="14" height="14" rx="1.5"/></svg>',
    close: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" ' +
      'stroke-width="2.2" stroke-linecap="round"><path d="M5 5l14 14M19 5L5 19"/></svg>',
  };

  /* 窗口控制图标：使用从官方截图 1:1 抠出的 PNG，原始尺寸显示 */
  var CONTROL_ICON = {
    settings: 'win-settings',
    minimize: 'win-minimize',
    maximize: 'win-minimize',
    close: 'win-close',
  };

  function renderWindowControls() {
    var host = document.getElementById('winctl');
    host.innerHTML = '';
    CFG.windowControls.forEach(function (name) {
      var b = el('button', name === 'close' ? 'is-close' : '');
      b.setAttribute('data-action', name);
      b.title = { settings: '设置', minimize: '最小化', maximize: '最大化', close: '关闭' }[name] || name;
      b.innerHTML = name === 'settings' ? CONTROL_SVG.settings :
        '<img src="assets/icons/' + (CONTROL_ICON[name] || 'win-close') + '.png" alt="' + (b.title) + '">';
      /* 齿轮：除了让 bridge 通知宿主，本地也把设置弹窗打开（假设宿主不接管） */
      if (name === 'settings') {
        b.addEventListener('click', function () {
          openSettings();
        });
      } else if (name === 'minimize' || name === 'maximize' || name === 'close') {
        /* 浏览器里没有宿主，点了要给出反馈，不能像坏了一样 */
        b.addEventListener('click', function () {
          if (!bridge.isWebView2) {
            toast(T('win.needHost', '窗口控制需要宿主 exe（浏览器里不可用）'), 'warn');
          }
        });
      }
      host.appendChild(b);
    });
  }

  /* ---------------- 左侧侧栏（官方 .hyp-side-bar）----------------
     顶部 logo（44x44 位于 (11.5,12)），底部游戏图标列表 + 分隔线 + 添加游戏按钮。
     官方几何：图标 36x36、间距 20、分隔线 margin-top 18、按钮 36x36。
  ------------------------------------------------ */
  function renderSidebar() {
    var S = CFG.sidebar || { games: [] };

    var lg = document.querySelector('#sidebarLogo img');
    if (lg && S.logo) lg.src = S.logo;

    var host = document.getElementById('sidebarBottom');
    host.innerHTML = '';

    /* 游戏图标列表 */
    var list = el('div', 'sidebar__games');
    (S.games || []).forEach(function (g) {
      var item = el('div', 'game-icon');
      item.setAttribute('data-side', g.key);
      item.setAttribute('data-tip', g.name || '');
      item.setAttribute('data-tip-place', 'right');
      /* 官方 .overlay 里放的是 .icon（20px 的下载箭头背景图）+ 进度环，
         默认【没有文字】。所以这里 progress 为空就不写文本，
         免得浮出一层金黄色字（实测官方该处是 (84,68,57) 而非黄色）。 */
      item.innerHTML =
        '<div class="game-icon__img"><img src="' + esc(g.icon) + '" alt="' + esc(g.name) + '">' +
        '<div class="game-icon__overlay' + (g.state === 'downloading' ? ' visible' : '') + '">'
        + (g.state === 'downloading' && g.progress ? esc(g.progress) : '') + '</div></div>' +
        '<div class="border"></div>' +
        (g.state === 'downloading'
          ? '<i class="game-icon__badge" title="有可用更新">' +
            '<svg viewBox="0 0 12 12"><path d="M6 1.5v6.5M3.2 5.4 6 8.2l2.8-2.8" ' +
            'fill="none" stroke="#16171a" stroke-width="1.7" ' +
            'stroke-linecap="round" stroke-linejoin="round"/></svg></i>'
          : '');
      item.addEventListener('click', function () {
        state.side = g.key;
        Array.prototype.forEach.call(list.children, function (c, k) {
          c.classList.toggle('on', (S.games[k] || {}).key === g.key);
        });
        var page = gameOf(g.key);
        if (page) {
          state.route = 'launcher';
          renderLauncher(g.key);
        } else {
          bridge.game.select(g.key);
        }
      });
      /* 右键：启动 / 定位 / 校验完整性 / 卸载 */
      item.addEventListener('contextmenu', function (e) {
        e.preventDefault();
        openGameMenu(item, g.key);
      });
      list.appendChild(item);
    });
    host.appendChild(list);


    /* 默认选中项 */
    var first = list.querySelector('.game-icon');
    if (first) first.classList.add('on');
  }

  /* ---------------- 右侧社交栏（官方 .hyp-game-social-media）----------------
     官方几何：容器 x=1228，每项 44x44，pitch 56，图标 36x36。
  ------------------------------------------------ */
  var SOCIAL_ICONS = {
    'social-home': '<path d="m3 10 9-7 9 7M5 9v12h5v-7h4v7h5V9"/>',
    'social-lobby': '<circle cx="9" cy="8" r="3"/><path d="M3 21v-3a6 6 0 0 1 12 0v3M16 5a3 3 0 0 1 0 6M21 21v-3a6 6 0 0 0-4-5.65"/>',
    'social-feedback': '<path d="M5 3h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H8l-5 3V5a2 2 0 0 1 2-2Z"/><path d="M12 7v5M12 15h.01"/>',
    'social-history': '<path d="M3 4v5h5M3 9a9 9 0 1 1 0 6"/><path d="M12 7v5l3 2"/>',
    'social-folder': '<path d="M3 7V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Zm0 3h18"/>',
  };

  function renderSocial() {
    var host = document.getElementById('social');
    host.innerHTML = '';
    (CFG.social || []).forEach(function (item) {
      var b = el('button', 'social-item');
      b.setAttribute('data-tip', item.label || '');
      b.setAttribute('data-tip-place', 'left');
      b.setAttribute('aria-label', item.label || '');
      b.innerHTML =
        (item.icon === 'social-qq' || item.icon === 'social-bilibili'
          ? '<span class="social-brand-icon ' + item.icon + '" aria-hidden="true"></span>'
          : SOCIAL_ICONS[item.icon]
          ? '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' + SOCIAL_ICONS[item.icon] + '</svg>'
          : '<img src="assets/icons/' + esc(item.icon) + '.png" alt="">') +
        (item.badge ? '<i class="badge"></i>' : '');
      b.addEventListener('click', function () {
        if (item.popup === 'account') { openAccountPopup(b); return; }
        /* href 用浏览器打开网页；action 交给宿主（打开日志目录、安装教程等本地功能） */
        if (item.href) {
          bridge.openExternal(item.href);
          toast(T('social.opening', '正在用浏览器打开') + '：' + (item.label || ''), 'info');
          return;
        }
        if (item.action) {
          if (item.action === 'lobby') { openLobby(); return; }
          bridge.settings.action(item.action);
          return;
        }
        if (item.panel) {
          /* 有内置面板的（更新日志/使用手册）走本地弹窗 */
          openModal(item.label || '提示', item.panel);
          return;
        }
        openModal(item.label || T('common.tip', '提示'),
          '<p>' + esc(T('social.todo', '这里可以接你自己的页面或宿主功能。')) + '</p>' +
          '<p>' + esc(T('social.hint', '在 config 里给这一项加 action 或 panel 字段即可生效。')) + '</p>');
      });
      host.appendChild(b);
    });
  }

  /* ---------------- 背景切换 ----------------
     dim = 背景之上的黑色遮罩不透明度（官方实测：Hub 0.221 / 启动页 0）
  ------------------------------------------------ */
  function applyBackground(game, dim, scrim, vignette) {
    var art = document.getElementById('bgArt');
    var title = document.getElementById('bgTitle');
    var bg = document.getElementById('bg');
    var sc = document.getElementById('bgScrim');
    var vg = document.getElementById('bgVignette');

    document.documentElement.style.setProperty(
      '--bg-dim', dim == null ? 0 : dim);
    if (sc) sc.classList.toggle('off', scrim === false);
    if (vg) {
      var v = vignette || (game && game.vignette) || '';
      if (v) {
        vg.src = v;
        vg.classList.remove('off');
      } else {
        vg.removeAttribute('src');
        vg.classList.add('off');
      }
    }

    bg.classList.add('bg-fading');
    setTimeout(function () {
      art.src = (game && game.background) || '';
      /* 背景视频：官方 .hyp-background-player__video，有就盖在底图上 */
      setVideo(game && game.video);
      if (game && game.titleOverlay) {
        title.src = game.titleOverlay;
        title.style.display = '';
      } else {
        title.removeAttribute('src');
        title.style.display = 'none';
      }
      bg.classList.remove('bg-fading');
    }, 180);
  }

  /* ---------------- 启动页 ---------------- */
  function renderLauncher(key) {
    var game = gameOf(key);
    if (!game) return renderHub();

    state.game = key;
    state.tab = 'activity';
    state.slide = 0;

    applyAccent(game);
    applyBackground(game, 0, game.scrim === true, game.vignette);
    bridge.game.select(key);

    var page = el('section', 'page page-launcher page-enter');

    /* --- 版本热点 chip ---
       官方 .hyp-info-button 里是【两张图叠着】（常态白底 / 悬停灰底），
       hover 时靠 :hover .hyp-image{translateY(-100%)} 整体上滚一格。
       所以这里必须渲染两张，只放一张的话悬停会把自己滚出去、底下空掉。 */
    /* 右侧立绘：放在页面最底层，按钮、资讯都压在它上面，不接收点击 */
    if (game.heroArt) {
      var hero = el('img', 'hero-art');
      hero.src = game.heroArt;
      hero.alt = '';
      hero.setAttribute('aria-hidden', 'true');
      var heroSlide = (game.carousel || [])[game.heroSlide || 0];
      if (heroSlide && heroSlide.detail) bindHero(hero, heroSlide);
      page.appendChild(hero);
    }

    var hotRow = el('div', 'hot-row');
    /* 没配 chip 就不显示（不再用占位图） */
    if (game.chip) {
      var chip = el('span', 'chip');
      var chipCfg = game.chip;
      var chipNormal = typeof chipCfg === 'string' ? chipCfg : chipCfg.normal;
      var chipHover = typeof chipCfg === 'string' ? null : chipCfg.hover;
      chipHover = chipHover || chipNormal;   /* 没配悬停态就复用常态，至少不会滑空 */
      chip.innerHTML =
        '<img src="' + esc(chipNormal) + '" alt="版本热点">' +
        '<img src="' + esc(chipHover) + '" alt="" aria-hidden="true">';
      chip.setAttribute('data-tip', '版本热点');
      chip.setAttribute('data-tip-place', 'top');
      chip.addEventListener('click', function () {
        var first = (game.carousel || [])[0];
        if (first && first.href) bridge.openExternal(first.href);
      });
      hotRow.appendChild(chip);
    }

    /* 播放/暂停按钮：官方仅在背景含视频时出现（.hyp-background-player__player） */
    if (game.pause) {
      var pause = el('button', 'pause-btn');
      pause.id = 'pauseBtn';
      pause.title = '暂停轮播';
      pause.innerHTML = '<svg viewBox="0 0 12 14"><rect x="0" y="0" width="4" height="14" rx="1"/>' +
        '<rect x="8" y="0" width="4" height="14" rx="1"/></svg>';
      pause.addEventListener('click', toggleCarousel);
      hotRow.appendChild(pause);
    }
    page.appendChild(hotRow);

    /* --- 操作面板：轮播 + 资讯列表共用一块面板（官方 .hyp-op-section，
           background: Mask-Secondary = rgba(0,0,0,.4)，border-radius:12px，
           实测 [128,380 384x290] = 轮播 178 + 资讯 112）--- */
    var panel = el('div', 'op-section');

    /* 活动轮播 */
    var slides = game.carousel || [];
    if (slides.length) {
      var car = el('div', 'carousel');
      var track = el('div', 'carousel-track');
      track.id = 'track';
      slides.forEach(function (s) {
        var sl = el('div', 'carousel-slide');
        sl.innerHTML = '<img src="' + esc(s.img) + '" alt="">';
        sl.addEventListener('click', function () {
          if (s.detail) openClassDetail(s);
          else if (s.href) bridge.openExternal(s.href);
        });
        track.appendChild(sl);
      });
      car.appendChild(track);

      var dots = el('div', 'carousel-dots');
      dots.id = 'dots';
      slides.forEach(function (_, i) {
        var d = el('i', i === 0 ? 'on' : '');
        d.addEventListener('click', function (e) {
          e.stopPropagation();
          goSlide(i);
          startCarousel(slides.length);   /* 手动切换后重新计时 */
        });
        dots.appendChild(d);
      });
      car.appendChild(dots);

      /* 左右切换：鼠标移到轮播上才显示 */
      if (slides.length > 1) {
        [['prev', -1, 'M10 3 5 8l5 5'], ['next', 1, 'M6 3l5 5-5 5']].forEach(function (b) {
          var nav = el('button', 'carousel-nav ' + b[0]);
          nav.setAttribute('aria-label', b[1] < 0 ? '上一张' : '下一张');
          nav.innerHTML = '<svg viewBox="0 0 16 16"><path d="' + b[2] + '" fill="none" stroke="currentColor" ' +
            'stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>';
          nav.addEventListener('click', function (e) {
            e.stopPropagation();
            goSlide(state.slide + b[1]);
            startCarousel(slides.length);
          });
          car.appendChild(nav);
        });
      }
      /* 鼠标停在轮播上时不自动切换 */
      car.addEventListener('mouseenter', function () { state.carouselHover = true; });
      car.addEventListener('mouseleave', function () { state.carouselHover = false; });
      panel.appendChild(car);
    }

    /* 资讯面板（官方 .hyp-notice-list：padding 16/16/12） */
    var TABS = [
      ['activity', '活动'],
      ['notice', '公告'],
      ['info', '资讯'],
    ];
    var notice = el('div', 'notice-list');

    var tabs = el('div', 'tabs');
    tabs.id = 'tabs';
    TABS.forEach(function (t) {
      var b = el('button', 'tab' + (t[0] === state.tab ? ' on' : ''), t[1]);
      b.setAttribute('data-tab', t[0]);
      b.addEventListener('click', function () {
        state.tab = t[0];
        Array.prototype.forEach.call(tabs.children, function (c) {
          c.classList.toggle('on', c.getAttribute('data-tab') === t[0]);
        });
        renderNews();
      });
      tabs.appendChild(b);
    });
    notice.appendChild(tabs);

    var news = el('ul', 'news');
    news.id = 'news';
    notice.appendChild(news);

    panel.appendChild(notice);
    page.appendChild(panel);

    /* --- 右下主按钮 ---
       三种态：
         未安装  disc = 下载箭头，品牌色底，文字「获取游戏」
         已安装  disc = 播放三角，品牌色底，文字「开始游戏」
         下载中  按钮切成 #0d0d0f 深底 + 白字，disc 换成 32px 圆环进度（圈内百分比） */
    var installed = !!game.installed;
    var cta = el('div', 'cta');
    var btn = el('button', 'btn-primary');
    btn.setAttribute('data-game', key);
    btn.setAttribute('data-action', installed && !game.updateTo ? 'launch' : 'download');
    btn.innerHTML =
      (installed && !game.updateTo
        ? '<span class="disc">' + CONTROL_PLAY_SVG + '</span>'
        : '<span class="disc"><i class="arrow"></i></span>') +
      '<span class="ring">' + ringSvg() + '</span>' +
      '<span class="label">' + esc(game.updateTo ? '立即更新' : installed
        ? ((game.cta && game.cta.launchText) || '开始游戏')
        : ((game.cta && game.cta.text) || '获取游戏')) + '</span>';

    /* 主按钮的状态机：失败→重试 / 待安装→安装 / 下载中⇄暂停 / 未安装→开始下载。
       这些分支都要 stopPropagation，否则会被 bridge 的 data-action 兜底再发一次。 */
    btn.addEventListener('click', function (e) {
      var g = gameOf(key) || {};
      var t = tasks[key];
      if (t) {
        if (t.phase === 'failed') {
          e.stopPropagation(); e.preventDefault(); taskRetry(key); return;
        }
        if (t.phase === 'ready') {
          e.stopPropagation(); e.preventDefault(); taskInstallNow(key); return;
        }
        if (t.phase === 'downloading' || t.phase === 'paused') {
          e.stopPropagation(); e.preventDefault();
          if (t.paused) taskResume(key); else taskPause(key);
          return;
        }
        return;
      }
      if (!g.installed || g.updateTo) {
        e.stopPropagation();
        e.preventDefault();
        taskStart(key, g.updateTo ? 'update' : 'install');
      }
    });

    cta.appendChild(btn);

    var sub = el('div', 'cta-sub', subRowFor(key));
    /* 已安装且有更新时，副行给一个次级入口「立即更新」 */
    sub.addEventListener('click', function (e) {
      if (e.target.closest('[data-act="lobby"]')) {
        e.preventDefault();
        e.stopPropagation();
        openLobby();
        return;
      }
      var a = e.target.closest('[data-act="update"]');
      if (!a) return;
      e.preventDefault();
      e.stopPropagation();
      taskStart(key, 'update');
    });
    cta.appendChild(sub);
    page.appendChild(cta);

    /* --- 下载任务列表（支持并发多任务，官方用 pending-box） ---
       容器常驻，内容由 taskRender 按 tasks 里的条目生成 */
    var tasksBox = el('div', 'tasks hide');
    tasksBox.id = 'tasks';
    page.appendChild(tasksBox);

    /* --- 底部进度 --- */
    if (game.progress) {
      page.appendChild(el('div', 'progress',
        esc(game.progress.text) + ' <b>' + esc(game.progress.done) + '/' + esc(game.progress.total) + '</b>'));
    }

    /* 背景切换器挂在 #bgSwitch（在 #view 之外），随时可渲染 */
    renderBgSwitcher(game);

    mount(page);
    /* 轮播要在挂载之后才启动：mount() 会先停掉旧的计时器（原外壳先启动再挂载，轮播从来不会自动切换）。
       宿主推送状态会重绘页面：轮播停在原来那张，不跳回第一张 */
    if (slides.length) {
      goSlide(state.slide);
      startCarousel(slides.length);
    }
    renderNews();
    /* 区服切换挂在 #view 里，必须等 mount 之后再插，否则会被 mount 清掉 */
    renderServerSwitcher(game);
    /* 挂载后再同步一次任务态：切页面/重进时正在下载的按钮要立刻显示进度 */
    taskRender();
  }

  /* 播放三角用 currentColor，颜色由 .btn-primary .disc 的 color 决定（= --brand）。
     写死 #fff 的话，换了浅色圆盘（--brand-ink）就会白底白三角看不见。 */
  var CONTROL_PLAY_SVG = '<svg viewBox="0 0 16 16" fill="currentColor">' +
    '<path d="M4 2.5l9 5.5-9 5.5z"/></svg>';

  function renderNews() {
    var host = document.getElementById('news');
    if (!host) return;
    var game = gameOf(state.game);
    var list = (game && game.news && game.news[state.tab]) || [];
    host.innerHTML = '';
    list.forEach(function (n) {
      var li = el('li', 'news-item');
      var title = window.towerReleaseNotes.title(n.title);
      li.innerHTML = '<span class="news-title">' + esc(title) + '</span>' +
        '<span class="news-date">' + esc(n.date) + '</span>';
      li.addEventListener('click', function () {
        if (n.action === 'releaseHistory') bridge.settings.action('releaseHistory');
        else if (n.href && n.href !== '#') bridge.openExternal(n.href);
        else openModal(title, '<p>' + esc(title) + '</p>' +
          '<p style="color:rgba(255,255,255,.45)">这里可以放公告正文，或者改成用宿主打开真实网页。</p>');
      });
      host.appendChild(li);
    });
  }

  /* ---------------- 轮播控制 ---------------- */
  function goSlide(i) {
    var track = document.getElementById('track');
    var dots = document.getElementById('dots');
    if (!track) return;
    var total = track.children.length;
    state.slide = (i + total) % total;
    track.style.transform = 'translateX(' + -state.slide * 100 + '%)';
    if (dots) {
      Array.prototype.forEach.call(dots.children, function (d, k) {
        d.classList.toggle('on', k === state.slide);
      });
    }
  }

  function startCarousel(total) {
    stopCarousel();
    if (total <= 1) return;
    carouselTimer = setInterval(function () {
      if (state.playing && !state.carouselHover) goSlide(state.slide + 1);
    }, 5000);
  }

  function stopCarousel() {
    if (carouselTimer) {
      clearInterval(carouselTimer);
      carouselTimer = null;
    }
  }

  function toggleCarousel() {
    state.playing = !state.playing;
    var b = document.getElementById('pauseBtn');
    if (b) {
      b.title = state.playing ? '暂停轮播' : '继续轮播';
      b.innerHTML = state.playing
        ? '<svg viewBox="0 0 12 14"><rect x="0" y="0" width="4" height="14" rx="1"/>' +
          '<rect x="8" y="0" width="4" height="14" rx="1"/></svg>'
        : '<svg viewBox="0 0 12 14" fill="#fff"><path d="M1 0l11 7-11 7z"/></svg>';
    }
  }

  /* ---------------- Hub 游戏选择页 ----------------
     几何全部实测自官方：logo (110,28)-(268,112)，简介左边界 128 行距 22，
     卡片行左边界 128 底边 y=664，未选中 124x125 选中 139x137 间距 20，
     游戏名 ink 落在 y=675，预约角标 48x24 贴卡片左上角。
  ------------------------------------------------ */
  function renderHub() {
    state.route = 'hub';
    applyAccent(null);

    var H = CFG.hub;
    var cur = H.entries[state.hubIndex] || H.entries[0] || {};
    applyBackground(
      { background: cur.background || H.background, titleOverlay: null },
      cur.bgDim != null ? cur.bgDim : H.bgDim,
      cur.scrim != null ? cur.scrim : H.scrim
    );

    var page = el('section', 'page page-hub page-enter');

    /* 首次进 Hub 先出骨架屏（官方 .hyp-el-skeleton），图片就绪后再换真内容 */
    if (!state.loaded) {
      var logo0 = cur.landLogo || H.logo;
      if (logo0) {
        page.innerHTML = '<div class="hub-logo"><img src="' + esc(logo0) + '" alt=""></div>';
      }
      page.insertAdjacentHTML('beforeend', hubSkeletonHtml());
      mount(page);
      setTimeout(function () {
        state.loaded = true;
        if (state.route === 'hub') renderHub();
      }, 420);
      return;
    }

    /* 左上角落地 logo（条目可选覆盖，官方元素 210x160） */
    var logo = cur.landLogo || H.logo;
    if (logo) {
      var lg = el('div', 'hub-logo');
      lg.innerHTML = '<img id="hubLogo" src="' + esc(logo) + '" alt="">';
      page.appendChild(lg);
    }

    /* 游戏简介（条目可选覆盖） */
    var desc = cur.description || H.description;
    if (desc) {
      var dc = el('div', 'hub-desc');
      dc.id = 'hubDesc';
      dc.textContent = desc;
      page.appendChild(dc);
    }

    /* 卡片行 */
    var row = el('div', 'hub-row');
    row.id = 'hubRow';
    H.entries.forEach(function (e, i) {
      var item = el('div', 'hub-item' + (i === state.hubIndex ? ' on' : ''));
      item.setAttribute('data-hub', e.key);
      item.innerHTML =
        '<img class="card-img" src="' + esc(e.card) + '" alt="' + esc(e.name) + '">' +
        (e.reserve ? '<span class="hub-reserve">预约</span>' : '') +
        '<div class="hub-name">' + esc(e.name) + '</div>';

      item.addEventListener('click', function () {
        selectHubEntry(i, true);
      });
      row.appendChild(item);
    });
    page.appendChild(row);

    mount(page);
    var first = row.querySelector('.hub-item.on');
    if (first) bridge.game.select(first.getAttribute('data-hub'));
  }

  /* 选中 Hub 里的某个游戏；enterPage=true 时若该游戏有启动页就直接进去 */
  function selectHubEntry(i, enterPage) {
    var H = CFG.hub;
    var e = H.entries[i];
    if (!e) return;
    state.hubIndex = i;

    var row = document.getElementById('hubRow');
    if (row) {
      Array.prototype.forEach.call(row.children, function (c, k) {
        c.classList.toggle('on', k === i);
      });
    }

    var logo = e.landLogo || H.logo;
    var lg = document.getElementById('hubLogo');
    if (lg && logo) lg.src = logo;

    var dc = document.getElementById('hubDesc');
    if (dc) dc.textContent = e.description || H.description;

    bridge.game.select(e.key);

    if (enterPage && e.page && gameOf(e.page)) {
      state.route = 'launcher';
      renderLauncher(e.page);
    } else if (!enterPage) {
      applyBackground(
        { background: e.background || H.background, titleOverlay: null },
        e.bgDim != null ? e.bgDim : H.bgDim,
        e.scrim != null ? e.scrim : H.scrim
      );
    }
  }

  /* ---------------- 挂载与路由 ---------------- */
  function mount(node) {
    stopCarousel();
    var view = document.getElementById('view');
    view.innerHTML = '';
    view.appendChild(node);
  }

  function goHub() {
    renderHub();
  }

  /* ---------------- 弹窗 ---------------- */
  function openModal(title, bodyHtml) {
    closeModal();
    var mask = el('div', 'modal-mask');
    mask.id = 'modalMask';
    var m = el('div', 'modal');
    m.innerHTML =
      '<div class="modal-head"><span>' + esc(title) + '</span>' +
      '<button class="modal-close">' + CONTROL_SVG.close + '</button></div>' +
      '<div class="modal-body scroll">' + bodyHtml + '</div>';
    mask.appendChild(m);
    mask.addEventListener('click', function (e) {
      if (e.target === mask) closeModal();
    });
    m.querySelector('.modal-close').addEventListener('click', closeModal);
    document.getElementById('app').appendChild(mask);
  }

  function showReleaseHistory(data) {
    var previous = document.getElementById('releaseHistory');
    if (!data.loading && !previous) return;
    var items = data.items || [];
    var html = '<section id="releaseHistory">';
    if (data.loading || !items.length) html += '<p role="status">' + esc(data.status || '') + '</p>';
    if (!items.length) html += '<p>暂无可显示的更新记录。</p>';
    items.forEach(function (item, index) {
      var published = item.date ? new Date(item.date) : null;
      var date = published && !isNaN(published.getTime()) ? published.toLocaleDateString('zh-CN') : '';
      var notes = window.towerReleaseNotes.render(item.notes || '此版本未提供更新说明。');
      html += '<details class="history-entry"' + (index === 0 ? ' open' : '') + '><summary>' +
        '<span class="history-title">' + esc(window.towerReleaseNotes.title(item.title)) + '</span>' +
        '<span class="history-meta">' +
        (date ? '<time>' + esc(date) + '</time>' : '') +
        (index === 0 ? '<span class="history-latest">最新记录</span>' : '') + '</span></summary>' +
        '<div class="release-notes">' + (notes || '<p>此版本未提供面向玩家的更新说明。</p>') + '</div></details>';
    });
    openModal('更新日志与历史', html + '</section>');
    var history = document.getElementById('releaseHistory');
    history.closest('.modal').classList.add('release-history-modal');
    history.addEventListener('click', function (event) {
      var link = event.target.closest('.release-notes a');
      if (!link) return;
      event.preventDefault();
      if (/^https?:\/\//i.test(link.href)) bridge.openExternal(link.href);
    });
  }

  /* 立绘点击：只认人物身上不透明的像素（图片四周的透明区域不算），点了弹出对应职业介绍 */
  function bindHero(hero, slide) {
    var canvas = null;
    function solid(e) {
      if (!hero.naturalWidth) return false;
      if (!canvas) {
        canvas = document.createElement('canvas');
        canvas.width = hero.naturalWidth;
        canvas.height = hero.naturalHeight;
        canvas.getContext('2d').drawImage(hero, 0, 0);
      }
      var r = hero.getBoundingClientRect();
      var x = Math.floor((e.clientX - r.left) / r.width * canvas.width);
      var y = Math.floor((e.clientY - r.top) / r.height * canvas.height);
      if (x < 0 || y < 0 || x >= canvas.width || y >= canvas.height) return false;
      try { return canvas.getContext('2d').getImageData(x, y, 1, 1).data[3] > 60; }
      catch (err) { return true; }   /* 读不了像素时退回整张图可点 */
    }
    hero.addEventListener('mousemove', function (e) { hero.classList.toggle('hot', solid(e)); });
    hero.addEventListener('mouseleave', function () { hero.classList.remove('hot'); });
    hero.addEventListener('click', function (e) {
      if (solid(e)) { e.stopPropagation(); openClassDetail(slide); }
    });
  }

  /* 轮播图的职业介绍：插画、定位、专属机制、起手装备与擅长的武器，底部可去项目主页 */
  function openClassDetail(slide) {
    var d = slide.detail;
    var row = function (label, value) {
      return '<div class="class-row"><span class="k">' + esc(label) + '</span><span class="v">' + esc(value) + '</span></div>';
    };
    openModal(d.name,
      '<div class="class-detail" style="--class-accent:' + esc(d.accent || '') + '">' +
      '<img class="class-banner" src="' + esc(slide.img) + '" alt="">' +
      row('定位', d.role) +
      '<div class="class-row mech"><span class="k">专属机制</span><span class="v"><b>' + esc(d.mechanic[0]) + '</b>　' +
        esc(d.mechanic[1]) + '</span></div>' +
      row('起手装备', d.kit) +
      row('擅长', d.bias) +
      '<p class="class-note">奖励里的武器约七成来自擅长的类型。</p>' +
      (slide.href ? '<a class="class-link" data-href="' + esc(slide.href) + '">项目主页与更新说明 →</a>' : '') +
      '</div>');
    var link = document.querySelector('#modalMask .class-link');
    if (link) link.addEventListener('click', function () { bridge.openExternal(link.getAttribute('data-href')); });
  }

  function closeModal() {
    var old = document.getElementById('modalMask');
    if (old) {
      if (old.querySelector('#lobbyDialog')) bridge.lobby.close();
      old.remove();
    }
  }

  /* ================= 联机大厅 =================
     房间列表由宿主用 Steam 大厅查询（TowerHost.LobbyListAsync），开房 / 加入时宿主写好无缝联机密码再启动游戏。
     lobbyState.view：'list' 房间列表 | 'host' 开房表单 */
  var lobbyState = { view: 'list', loading: false, error: '', rooms: [], version: '', joining: null, mine: null };

  function openLobby() {
    closeModal();
    lobbyState = { view: 'list', loading: true, error: '', rooms: [], version: '', joining: null };
    var mask = el('div', 'modal-mask');
    mask.id = 'modalMask';
    var dlg = el('div', 'set-dialog lobby-dialog');
    dlg.id = 'lobbyDialog';
    mask.appendChild(dlg);
    mask.addEventListener('click', function (e) { if (e.target === mask) closeModal(); });
    document.getElementById('app').appendChild(mask);
    renderLobby();
    bridge.lobby.list();
  }

  function renderLobby() {
    var dlg = document.getElementById('lobbyDialog');
    if (!dlg) return;
    var L = lobbyState;
    var html = '<button class="set-close" data-lobby="close" aria-label="关闭">' +
      '<svg viewBox="0 0 14 14"><path d="M1 1l12 12M13 1L1 13" stroke="currentColor" stroke-width="1.6"/></svg></button>' +
      '<div class="lobby-body"><div class="card-header">联机大厅</div>';
    if (L.view === 'host') {
      html += '<div class="lobby-hint">开房后启动器会自动设置无缝联机密码并启动游戏；进游戏后照常用无缝联机的道具开启联机，房间就会出现在大厅里，队友不用填密码。</div>' +
        '<div class="set-row"><div class="label">房间名</div><div class="ctrl"><input class="inp" id="lobbyName" maxlength="32" placeholder="例如：一起爬塔"></div></div>' +
        '<div class="set-row"><div class="label">人数上限</div><div class="ctrl"><select class="inp" id="lobbyMax">' +
        [2, 3, 4, 5, 6, 7, 8].map(function (n) { return '<option value="' + n + '"' + (n === 4 ? ' selected' : '') + '>' + n + ' 人</option>'; }).join('') +
        '</select></div></div>' +
        '<div class="set-row"><div class="label">房间密码<div class="hint">可不填；填了就是私密房间，只有知道密码的人能进</div></div>' +
        '<div class="ctrl"><input class="inp" id="lobbyPassword" maxlength="32" type="password" placeholder="不填即公开"></div></div>' +
        '<div class="lobby-actions"><button class="btn-ghost" data-lobby="back">返回</button>' +
        '<button class="btn-ghost primary" data-lobby="create">开房并启动游戏</button></div>';
    } else {
      html += '<div class="lobby-toolbar"><span class="lobby-hint">' +
        (L.version ? '你的版本 ' + esc(L.version) + ' · 版本不同的房间不能加入' : '同一版本的队友开的房间会显示在这里') + '</span>' +
        '<span><button class="btn-ghost" data-lobby="refresh"' + (L.loading ? ' disabled' : '') + '>' + (L.loading ? '刷新中…' : '刷新') + '</button> ' +
        '<button class="btn-ghost primary" data-lobby="host">开房</button></span></div>';
      if (L.mine) {
        html += '<div class="lobby-mine lobby-mine-' + esc(L.mine.state) + (L.mine.state === 'open' && !L.mine.ready ? ' lobby-mine-wait' : '') + '">你开的房间「' + esc(L.mine.name) + '」：' + esc(L.mine.detail) +
          (L.mine.state === 'open' ? '<span class="lobby-hint">（自己的房间在下面的列表里可能看不到，请让队友刷新确认）</span>' : '') + '</div>';
      }
      if (L.error) {
        html += '<div class="lobby-empty">' + esc(L.error) + '</div>';
      } else if (L.loading && !L.rooms.length) {
        html += '<div class="lobby-empty">正在向 Steam 查询房间…</div>';
      } else if (!L.rooms.length) {
        html += '<div class="lobby-empty">暂时没有房间，点「开房」开一个吧</div>';
      } else {
        html += '<ul class="lobby-list">' + L.rooms.map(function (r) {
          var blocked = !r.sameVersion ? '版本 ' + esc(r.version) : (r.full ? '已满' : (r.started ? '旅程中' : ''));
          var joining = L.joining === r.id;
          return '<li class="lobby-room' + (blocked ? ' off' : '') + '">' +
            '<div class="lobby-main"><div class="lobby-name">' + (r.locked ? '🔒 ' : '') + esc(r.name) + '</div>' +
            '<div class="lobby-meta">房主 ' + esc(r.host || '—') + ' · ' + esc(r.state || '大赐福') +
            (r.open ? '' : ' · <span class="lobby-wait">等待房主开启联机</span>') + '</div></div>' +
            '<div class="lobby-count">' + r.players + ' / ' + r.max + '</div>' +
            (joining
              ? '<div class="lobby-join"><input class="inp lobby-pass" id="lobbyJoinPass" type="password" maxlength="32" placeholder="房间密码">' +
                '<button class="btn-ghost primary" data-lobby="joinConfirm" data-id="' + esc(r.id) + '">加入</button></div>'
              : '<button class="btn-ghost' + (blocked ? '' : ' primary') + '" data-lobby="join" data-id="' + esc(r.id) + '"' +
                (blocked ? ' disabled' : '') + '>' + (blocked || '加入') + '</button>') +
            '</li>';
        }).join('') + '</ul>';
      }
    }
    html += '</div>';
    dlg.innerHTML = html;
    var focus = dlg.querySelector('#lobbyJoinPass') || dlg.querySelector('#lobbyName');
    if (focus) focus.focus();
  }

  document.addEventListener('click', function (e) {
    var b = e.target.closest('[data-lobby]');
    if (!b || !document.getElementById('lobbyDialog')) return;
    var act = b.getAttribute('data-lobby');
    var L = lobbyState;
    if (act === 'close') { closeModal(); return; }
    if (act === 'refresh') { L.loading = true; L.error = ''; renderLobby(); bridge.lobby.list(); return; }
    if (act === 'host') { L.view = 'host'; renderLobby(); return; }
    if (act === 'back') { L.view = 'list'; renderLobby(); return; }
    if (act === 'create') {
      var name = (document.getElementById('lobbyName').value || '').trim();
      if (!name) { toast('请填写房间名', 'warn'); return; }
      bridge.lobby.host({
        name: name,
        max: parseInt(document.getElementById('lobbyMax').value, 10) || 4,
        password: document.getElementById('lobbyPassword').value || '',
      });
      closeModal();
      return;
    }
    var id = b.getAttribute('data-id');
    var room = L.rooms.filter(function (r) { return r.id === id; })[0];
    if (!room) return;
    if (act === 'join') {
      if (room.locked) { L.joining = id; renderLobby(); return; }
      bridge.lobby.join(id, '');
      closeModal();
      return;
    }
    if (act === 'joinConfirm') {
      var pass = document.getElementById('lobbyJoinPass').value || '';
      if (!pass) { toast('请输入房间密码', 'warn'); return; }
      bridge.lobby.join(id, pass);
      closeModal();
    }
  });
  document.addEventListener('keydown', function (e) {
    if (e.key !== 'Enter' || !document.getElementById('lobbyDialog')) return;
    var confirm = document.querySelector('[data-lobby="joinConfirm"]') || document.querySelector('[data-lobby="create"]');
    if (confirm) confirm.click();
  });

  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape') closeModal();
  });

  /* ---------------- 初始化 ---------------- */
  /* ================= 设置弹窗 =================
     结构与几何对齐官方 .hyp-setting-dialog / .hyp-settings：
       左侧 .hyp-settings__menu（239px，深一层底）
       右侧 .hyp-settings__content（标题 18px，行距按 Element Plus 的 14/20）
     内容全部由 config 的 settings 段驱动，两套配置各自不同。
     逻辑暂未实装：改值只更新本地状态并重绘，方便先看效果。
     ============================================ */
  var setState = {};      /* 当前值 */
  var setTab = null;      /* 当前分类 */

  function settingsCfg() {
    return CFG.settings || { title: '设置', tabs: [] };
  }

  /* 任何设置项变动都走这里：落盘 -> 本地生效 -> 通知宿主 */
  function onSettingChange(key, value) {
    if (!key) return;
    var patch = {};
    patch[key] = value;
    /* 无缝联机的设置（含联机密码）只写游戏目录里的配置，不留在页面存储里 */
    if (key.indexOf('ersc.') !== 0) saveStore(patch);
    setState[key] = value;
    loadRuntime();
    applyLocalSettings();
    bridge.settings.change(key, value);
  }

  function initSetState() {
    setState = {};
    var saved = loadStore();
    (settingsCfg().tabs || []).forEach(function (tb) {
      (tb.groups || []).forEach(function (g) {
        (g.rows || []).forEach(function (r) {
          /* 配置里的 value 是默认值；本地存过的优先（这就是持久化生效的地方） */
          if (r.key) {
            setState[r.key] = (r.key in saved) ? saved[r.key] : r.value;
          }
        });
      });
    });
  }

  function rowControl(r) {
    var box = el('div', 'ctrl');
    var v = r.key ? setState[r.key] : r.value;

    if (r.type === 'switch') {
      var sw = el('button', 'sw' + (v ? ' on' : ''));
      sw.setAttribute('role', 'switch');
      sw.setAttribute('aria-checked', v ? 'true' : 'false');
      sw.addEventListener('click', function () {
        setState[r.key] = !setState[r.key];
        sw.classList.toggle('on', !!setState[r.key]);
        sw.setAttribute('aria-checked', setState[r.key] ? 'true' : 'false');
        onSettingChange(r.key, setState[r.key]);
      });
      box.appendChild(sw);

    } else if (r.type === 'check') {
      /* 复选框：label 在右侧 */
      var cb = el('label', 'cbx');
      cb.innerHTML =
        '<input type="checkbox"' + (v ? ' checked' : '') + '>' +
        '<span class="box"><svg viewBox="0 0 12 12"><path d="M2 6.4l2.6 2.6L10 3.4" ' +
        'fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" ' +
        'stroke-linejoin="round"/></svg></span>' +
        (r.text ? '<span>' + esc(r.text) + '</span>' : '');
      var inp = cb.querySelector('input');
      inp.addEventListener('change', function () {
        setState[r.key] = inp.checked;
        onSettingChange(r.key, inp.checked);
      });
      box.appendChild(cb);

    } else if (r.type === 'input') {
      var tx = el('input', 'inp');
      tx.type = r.numeric ? 'number' : 'text';
      if (r.numeric) { tx.min = r.min; tx.max = r.max; tx.step = 1; }
      tx.value = v == null ? '' : v;
      if (r.placeholder) tx.placeholder = r.placeholder;
      tx.addEventListener('change', function () {
        if (r.numeric && (!/^\d+$/.test(tx.value) || +tx.value < r.min || +tx.value > r.max)) {
          toast('请输入 ' + r.min + '～' + r.max + ' 的整数百分比', 'warn');
          return;
        }
        setState[r.key] = tx.value;
        onSettingChange(r.key, tx.value);
      });
      tx.addEventListener('keydown', function (e) {
        if (e.key === 'Enter') { e.preventDefault(); tx.blur(); }
      });
      box.appendChild(tx);

    } else if (r.type === 'select') {
      var sel = el('div', 'sel');
      var btn = el('button', 'sel-btn', esc(String(v)));
      var list = el('div', 'sel-list');
      (r.options || []).forEach(function (o) {
        var it = el('button', 'sel-item' + (o === v ? ' on' : ''), esc(String(o)));
        it.addEventListener('click', function (e) {
          e.stopPropagation();
          setState[r.key] = o;
          btn.textContent = o;
          sel.classList.remove('open');
          Array.prototype.forEach.call(list.children, function (c) {
            c.classList.toggle('on', c.textContent === o);
          });
          onSettingChange(r.key, o);
        });
        list.appendChild(it);
      });
      btn.addEventListener('click', function (e) {
        e.stopPropagation();
        var was = sel.classList.contains('open');
        document.querySelectorAll('.sel.open').forEach(function (s) {
          s.classList.remove('open');
        });
        if (!was) sel.classList.add('open');
      });
      sel.appendChild(btn);
      sel.appendChild(list);
      box.appendChild(sel);

    } else if (r.type === 'seg') {
      var seg = el('div', 'seg');
      (r.options || []).forEach(function (o) {
        var b = el('button', o === v ? 'on' : '', esc(String(o)));
        b.addEventListener('click', function () {
          setState[r.key] = o;
          Array.prototype.forEach.call(seg.children, function (c) {
            c.classList.toggle('on', c === b);
          });
          onSettingChange(r.key, o);
        });
        seg.appendChild(b);
      });
      box.appendChild(seg);

    } else if (r.type === 'slider') {
      var sl = el('input', 'slider');
      sl.type = 'range';
      sl.min = r.min != null ? r.min : 0;
      sl.max = r.max != null ? r.max : 100;
      sl.value = v;
      var num = el('span', 'value', v + (r.unit || ''));
      sl.addEventListener('input', function () {
        num.textContent = sl.value + (r.unit || '');
        setState[r.key] = +sl.value;
      });
      box.appendChild(sl);
      box.appendChild(num);

    } else if (r.type === 'button') {
      var bb = el('button', 'btn-ghost' + (r.primary ? ' primary' : ''), esc(r.text || ''));
      bb.addEventListener('click', function () {
        bridge.settings && bridge.settings.action && bridge.settings.action(r.action);
        if (r.href) bridge.openExternal(r.href);
        /* 本地也跑一遍，没宿主时能看到效果 */
        if (r.action === 'checkUpdate') checkUpdates();
        if (r.action === 'clearCache') {
          setState.cacheUsed = '0 B';
          renderSettings();
        }
      });
      box.appendChild(bb);

    } else if (r.type === 'buttons') {
      (r.items || []).forEach(function (it) {
        var b2 = el('button', 'btn-ghost' + (it.primary ? ' primary' : ''), esc(it.text || ''));
        b2.addEventListener('click', function () {
          bridge.settings && bridge.settings.action && bridge.settings.action(it.action);
          if (it.href) bridge.openExternal(it.href);
          if (it.action === 'checkUpdate') checkUpdates();
          if (it.action === 'clearCache') {
            setState.cacheUsed = '0 B';
            renderSettings();
          }
        });
        box.appendChild(b2);
      });

    } else if (r.type === 'static') {
      box.appendChild(el('span', 'value', esc(String(v))));
    }
    return box;
  }

  /* 额外 DLL 插件列表：数据由宿主推送（setState.plugins），开关与移除交回宿主处理 */
  function pluginRows(grp) {
    var list = setState.plugins || [];
    if (!list.length) {
      grp.appendChild(el('div', 'set-row', '<div class="label">还没有插件<span class="hint">' +
        '用下面的按钮添加；插件会复制进模组目录的 plugins 文件夹，更新不会动它</span></div>'));
      return;
    }
    list.forEach(function (p) {
      var row = el('div', 'set-row');
      row.appendChild(el('div', 'label', esc(p.name) + '<span class="hint">' +
        esc(p.exists ? p.path : p.path + '（文件不见了，启动时会跳过）') + '</span>'));
      var box = el('div', 'ctrl');
      var sw = el('button', 'sw' + (p.enabled ? ' on' : ''));
      sw.setAttribute('role', 'switch');
      sw.setAttribute('aria-checked', p.enabled ? 'true' : 'false');
      sw.setAttribute('aria-label', '启用 ' + p.name);
      sw.addEventListener('click', function () {
        p.enabled = !p.enabled;
        sw.classList.toggle('on', p.enabled);
        sw.setAttribute('aria-checked', p.enabled ? 'true' : 'false');
        bridge.settings.action('togglePlugin', { path: p.path, enabled: p.enabled });
      });
      var rm = el('button', 'btn-ghost', '移除');
      rm.addEventListener('click', function () {
        bridge.settings.action('removePlugin', { path: p.path });
      });
      box.appendChild(sw);
      box.appendChild(rm);
      row.appendChild(box);
      grp.appendChild(row);
    });
  }

  function aboutBlock() {
    var S = settingsCfg();
    var wrap = el('div', '');
    var b = el('div', 'about-block');
    var mark = S.appIcon ? esc(S.appIcon) : 'assets/icons/logo-app.svg';
    b.innerHTML =
      '<img class="mark" src="' + mark + '" alt="">' +
      '<div><div class="name">' + esc(S.appName || CFG.app.name || '') + '</div>' +
      '<div class="ver">整合包 ' + esc(setState.version || '—') + ' · 启动器 ' +
      esc(setState.launcherVersion || '—') + '</div></div>';
    wrap.appendChild(b);
    if (S.links && S.links.length) {
      var lk = el('div', 'about-links');
      lk.innerHTML = S.links.map(function (l) {
        return '<a data-href="' + esc(l.href || '') + '" data-act="' + esc(l.action || '') + '">' +
          esc(l.label) + '</a>';
      }).join('　·　');
      lk.addEventListener('click', function (e) {
        var a = e.target.closest('a');
        if (!a) return;
        if (a.getAttribute('data-href')) bridge.openExternal(a.getAttribute('data-href'));
        else if (a.getAttribute('data-act')) bridge.settings.action(a.getAttribute('data-act'));
      });
      wrap.appendChild(lk);
    }
    if (S.copyright) {
      var cp = el('div', 'about-links');
      cp.style.marginTop = '10px';
      cp.textContent = S.copyright;
      wrap.appendChild(cp);
    }
    return wrap;
  }

  function renderSettings() {
    var S = settingsCfg();
    var tabs = S.tabs || [];
    if (!setTab || !tabs.some(function (t) { return t.key === setTab; })) {
      setTab = tabs.length ? tabs[0].key : null;
    }

    var dlg = document.getElementById('setDialog');
    if (!dlg) return;
    dlg.innerHTML = '';

    /* 左侧菜单 */
    var menu = el('div', 'set-menu');
    menu.innerHTML = '<div class="card-header">' + esc(S.title || '设置') + '</div>';
    tabs.forEach(function (t) {
      var b = el('button', 'set-tab' + (t.key === setTab ? ' on' : ''),
        esc(t.name) + (t.dot ? '<i class="dot"></i>' : ''));
      if (t.dot) b.innerHTML = esc(t.name) + '<i class="dot"></i>';
      b.addEventListener('click', function () {
        setTab = t.key;
        renderSettings();
      });
      menu.appendChild(b);
    });
    dlg.appendChild(menu);

    /* 右侧内容 */
    var cur = tabs.filter(function (t) { return t.key === setTab; })[0] || {};
    var content = el('div', 'set-content');
    content.innerHTML = '<div class="card-header">' + esc(cur.name || '') + '</div>';
    var body = el('div', 'set-body');

    (cur.groups || []).forEach(function (g) {
      var grp = el('div', 'set-group');
      if (g.title) grp.appendChild(el('div', 'set-group-title', esc(g.title)));
      (g.rows || []).forEach(function (r) {
        if (r.type === 'about') {
          grp.appendChild(aboutBlock());
          return;
        }
        if (r.type === 'plugins') {
          pluginRows(grp);
          return;
        }
        var row = el('div', 'set-row');
        var lab = el('div', 'label', esc(r.label) +
          (r.hint ? '<span class="hint">' + esc(r.hint) + '</span>' : ''));
        row.appendChild(lab);
        row.appendChild(rowControl(r));
        grp.appendChild(row);
      });
      body.appendChild(grp);
    });
    content.appendChild(body);
    dlg.appendChild(content);

    /* 右上关闭 */
    var cl = el('button', 'set-close');
    cl.innerHTML = CONTROL_SVG.close;
    cl.addEventListener('click', closeModal);
    dlg.appendChild(cl);
  }

  function openSettings() {
    setTab = null;
    closeModal();
    var mask = el('div', 'modal-mask');
    mask.id = 'modalMask';
    var dlg = el('div', 'set-dialog');
    dlg.id = 'setDialog';
    mask.appendChild(dlg);
    mask.addEventListener('click', function (e) {
      if (e.target === mask) closeModal();
      document.querySelectorAll('.sel.open').forEach(function (s) {
        if (!s.contains(e.target)) s.classList.remove('open');
      });
    });
    document.getElementById('app').appendChild(mask);
    renderSettings();
    bridge.game.queryStatus(state.game);
  }

  /* ================= 下载 / 更新 =================
     状态机：
       idle ──点按钮──► downloading ⇄ paused ──100%──► verifying ──► extracting
            ──► installing ──► finished(installed)
     页面负责 UI；真正干活交给宿主（bridge.game.download / pause / resume / cancel）。
     没有宿主时用定时器自己模拟进度，方便先看效果。
     宿主可用 onMessage 回推 { type:'game', action:'progress' } 覆盖模拟值。
     ============================================== */
  var tasks = {};          /* gameKey -> task */
  var taskTimer = null;
  var simulate = true;     /* 宿主回 {type:'app',action:'hostTakeover',payload:{download:true}} 可关掉 */

  var PHASE_TEXT = {
    queued: '排队中',
    downloading: '下载中',
    paused: '已暂停',
    ready: '待安装',
    verifying: '校验中',
    extracting: '解压中',
    installing: '安装中',
    finished: '已完成',
    failed: '下载失败',
  };

  function gb(n) {
    var u = ['B', 'KB', 'MB', 'GB', 'TB'];
    var i = 0;
    while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
    return (i >= 3 ? n.toFixed(1) : Math.round(n)) + ' ' + u[i];
  }

  function mbps(b) { return (b / 1048576).toFixed(1) + ' MB/s'; }

  function mmss(sec) {
    if (!isFinite(sec) || sec <= 0) return '--:--';
    sec = Math.round(sec);
    var m = Math.floor(sec / 60);
    var s = sec % 60;
    if (m < 60) return m + ' 分 ' + (s < 10 ? '0' : '') + s + ' 秒';
    return Math.floor(m / 60) + ' 小时 ' + (m % 60) + ' 分';
  }

  function gameSize(key) {
    var g = gameOf(key) || {};
    return (g.size || 8) * 1073741824;
  }

  /* 官方该状态不需要额外标题叠层时用它清空视频 */
  function setVideo(src) {
    var v = document.getElementById('bgVideo');
    if (!v) return;
    if (src) {
      if (v.getAttribute('data-src') !== src) {
        v.setAttribute('data-src', src);
        v.src = src;
      }
      v.classList.remove('off');
      var pr = v.play();
      if (pr && pr.catch) pr.catch(function () { /* 自动播放被拦，静默 */ });
    } else {
      v.pause();
      v.classList.add('off');
    }
  }

  /* 主按钮下面那行的内容，随安装/更新状态变 */
  function subRowFor(key) {
    var g = gameOf(key) || {};
    if (g.installed) {
      if (g.updateTo) {
        return '新版本 ' + esc(g.updateTo);
      }
      return '';
    }
    /* 模组装在启动器所在文件夹，没有「定位已有安装」这一说，只显示配置里的提示 */
    return esc((g.cta && g.cta.hint) || '');
  }

  function taskStart(key, kind, opts) {
    opts = opts || {};
    var total = gameSize(key);
    if (kind === 'update') {
      total = ((gameOf(key) || {}).patchSize || 1.2) * 1073741824;
    }
    /* 模拟速度按容量自适应，让演示大约 30 秒跑完（真实实现里这个值由宿主持有） */
    var base = total / 30;
    /* 并发上限：设置里的「同时下载任务数」真的会生效 */
    var queued = runningCount() >= runtime.maxTask;
    tasks[key] = {
      key: key,
      kind: kind || 'install',
      phase: queued ? 'queued' : 'downloading',
      pre: !!opts.pre,            /* 预下载：下完不自动安装 */
      total: total,
      done: 0,
      percent: 0,
      base: base,
      speed: base * (0.8 + Math.random() * 0.4),
      paused: false,
      enter: true,
      startedAt: Date.now(),
    };
    if (queued) toast(T('task.queued', '已加入队列，等前面的下载完成'), 'info');
    bridge.game.download(key);
    ensureTimer();
    taskRender();
  }

  function taskPause(key) {
    var t = tasks[key];
    if (!t) return;
    t.paused = true;
    t.phase = 'paused';
    bridge.game.pause(key);
    taskRender();
  }

  function taskResume(key) {
    var t = tasks[key];
    if (!t) return;
    t.paused = false;
    t.phase = 'downloading';
    bridge.game.resume(key);
    taskRender();
  }

  function taskCancel(key) {
    var t = tasks[key];
    if (t) { t.leaving = true; }
    delete tasks[key];
    bridge.game.cancel(key);
    if (taskTimer) { clearInterval(taskTimer); taskTimer = null; }
    taskRender();
    setTimeout(function () { taskRender(); }, 220);
    if (state.route === 'launcher' && state.game === key) renderLauncher(key);
  }

  /* ---- 失败与重试 ----
     真实下载必然遇到断网/校验失败，这里给出完整的失败态 + 重试。
     模拟里不会随机失败（免得演示时莫名报错），
     宿主回 { type:'game', action:'progress', payload:{ phase:'failed', error:'…' } }
     或用 hoyoApp.taskFail(key, '原因') 都能进入这个状态。 */
  function taskFail(key, reason) {
    var t = tasks[key];
    if (!t) return;
    t.phase = 'failed';
    t.error = reason || '网络中断';
    toast((gameOf(key) || {}).name || key, 'error');
    bridge.game.flush(key);
    taskRender();
  }

  function taskRetry(key) {
    var t = tasks[key];
    if (!t) return;
    /* 保留已下载的部分（真实实现里从这里续传） */
    t.phase = 'downloading';
    t.error = null;
    t.paused = false;
    t.percent = Math.min(t.percent, 100);
    t.done = t.percent / 100 * t.total;
    /* 宿主接管时重新发起（宿主那边已下好、校验过的数据会跳过） */
    if (!simulate) bridge.game.download(key);
    ensureTimer();
    taskRender();
  }

  /* ---- 预下载：先下好，等用户点安装 ---- */
  function taskPreDownload(key) {
    taskStart(key, (gameOf(key) || {}).updateTo ? 'update' : 'install', { pre: true });
    toast(T('task.preStarted', '已加入预下载，完成后可手动安装'), 'info');
  }

  /* 预下载完成但还没安装时，把按钮切成「安装」 */
  function taskInstallNow(key) {
    var t = tasks[key];
    if (!t) return;
    t.phase = 'installing';
    t.phaseLeft = 1.4;
    bridge.game.install(key);
    ensureTimer();
    taskRender();
  }

  /* 正在下载的任务数（用于并发上限） */
  function runningCount() {
    return Object.keys(tasks).filter(function (k) {
      var t = tasks[k];
      return t.phase === 'downloading' && !t.paused;
    }).length;
  }

  /* 下载/安装完成：标记已安装、清掉侧栏图标上的进度，稍后移除任务卡片 */
  function taskFinish(k) {
    var t = tasks[k];
    if (!t) return;
    t.phase = 'finished';
    t.percent = 100;
    var g = gameOf(k);
    if (g) { g.installed = true; g.updateTo = null; }
    var icon = document.querySelector('.game-icon[data-side="' + k + '"]');
    if (icon) {
      var ov = icon.querySelector('.game-icon__overlay');
      if (ov) { ov.classList.remove('visible'); ov.innerHTML = ''; }
      var bd = icon.querySelector('.game-icon__badge');
      if (bd) bd.remove();
    }
    setTimeout(function () { delete tasks[k]; taskRender(); }, 900);
    taskRender();
  }

  function ensureTimer() {
    if (taskTimer) return;
    var last = Date.now();
    taskTimer = setInterval(function () {
      var now = Date.now();
      var dt = (now - last) / 1000;
      last = now;
      var dirty = false;

      Object.keys(tasks).forEach(function (k) {
        var t = tasks[k];
        if (t.enter) t.enter = false;

        /* 排队中的任务：有空位就开跑 */
        if (t.phase === 'queued') {
          if (runningCount() < runtime.maxTask) { t.phase = 'downloading'; dirty = true; }
        }

        if (t.phase === 'downloading' && simulate && !t.paused) {
          /* 速度围绕基准抖动，模拟真实网络 */
          var bs = t.base || (t.total / 30);
          t.speed = Math.max(bs * 0.35, Math.min(bs * 1.6,
            t.speed + (Math.random() - 0.48) * bs * 0.3));
          /* 限速：设置里填了 MB/s 就真的压下来（并发时按任务数分摊） */
          var lim = runtime.limitSpeed;
          if (lim > 0) {
            var share = lim * 1048576 / Math.max(1, runningCount());
            t.speed = Math.min(t.speed, share);
          }
          t.done = Math.min(t.total, t.done + t.speed * dt);
          t.percent = t.done / t.total * 100;
          if (t.percent >= 100) {
            t.percent = 100;
            if (t.pre) {
              /* 预下载：下完停在「待安装」，等用户点 */
              t.phase = 'ready';
              toast((gameOf(k) || {}).name + ' ' +
                T('task.preDone', '预下载完成，可以安装了'), 'success');
            } else {
              t.phase = 'verifying';
              t.phaseLeft = 1.2;
            }
          }
          dirty = true;

        } else if (simulate && (t.phase === 'verifying' || t.phase === 'extracting' ||
                   t.phase === 'installing')) {
          /* 宿主接管时阶段由宿主推送（finished 走 taskFinish），这里只给模拟用 */
          t.phaseLeft -= dt;
          dirty = true;
          if (t.phaseLeft <= 0) {
            if (t.phase === 'verifying') { t.phase = 'extracting'; t.phaseLeft = 1.8; }
            else if (t.phase === 'extracting') { t.phase = 'installing'; t.phaseLeft = 1.4; }
            else taskFinish(k);
          }
        }
      });

      if (dirty) taskRender();
      if (!Object.keys(tasks).length && taskTimer) {
        clearInterval(taskTimer);
        taskTimer = null;
      }
    }, 250);
  }

  function ringSvg() {
    var C = 2 * Math.PI * 13.2;
    return '<svg viewBox="0 0 32 32">' +
      '<circle class="track" cx="16" cy="16" r="13.2"/>' +
      '<circle class="bar" cx="16" cy="16" r="13.2" ' +
      'stroke-dasharray="' + C.toFixed(2) + '" stroke-dashoffset="' + C.toFixed(2) + '"/>' +
      '</svg><span class="pct">0%</span><span class="ico"></span>';
  }

  function taskRender() {
    var keys = Object.keys(tasks);
    var active = keys.length ? tasks[keys[0]] : null;
    var btn = document.querySelector('.btn-primary');

    if (btn) {
      var key = btn.getAttribute('data-game');
      var t = tasks[key];
      var on = !!t && t.phase !== 'finished';
      btn.classList.toggle('is-busy', on);
      btn.classList.toggle('is-paused', !!t && t.paused);
      btn.classList.toggle('entering', !!t && t.enter);

      var ring = btn.querySelector('.ring');
      var label = btn.querySelector('.label');

      if (on && ring) {
        var pct = Math.round(t.percent);
        ring.querySelector('.pct').textContent = pct + '%';
        ring.classList.toggle('hundred', pct >= 100);
        var C = 2 * Math.PI * 13.2;
        ring.querySelector('.bar')
          .setAttribute('stroke-dashoffset', (C * (1 - t.percent / 100)).toFixed(2));
        ring.classList.toggle('blink', t.phase === 'downloading' && !t.paused);
        ring.querySelector('.ico').innerHTML = t.paused
          ? '<svg viewBox="0 0 16 16"><path d="M4 2.5l10 5.5-10 5.5z"/></svg>'
          : '<svg viewBox="0 0 16 16"><rect x="3" y="2.5" width="4" height="11" rx="1"/>' +
            '<rect x="9" y="2.5" width="4" height="11" rx="1"/></svg>';
        /* 失败 / 待安装 / 排队时圆环没意义，换成对应图形 */
        if (t.phase === 'failed') {
          ring.querySelector('.bar').setAttribute('stroke-dashoffset', C.toFixed(2));
          ring.querySelector('.ico').innerHTML =
            '<svg viewBox="0 0 16 16"><path d="M13 8a5 5 0 1 1-1.5-3.6" fill="none" ' +
            'stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>' +
            '<path d="M13 3.2v3h-3" fill="none" stroke="currentColor" stroke-width="1.8" ' +
            'stroke-linecap="round" stroke-linejoin="round"/></svg>';
          ring.style.opacity = '1';
        } else if (t.phase === 'ready') {
          ring.querySelector('.ico').innerHTML =
            '<svg viewBox="0 0 16 16"><path d="M8 2v7M5 6.2 8 9.2l3-3" fill="none" ' +
            'stroke="currentColor" stroke-width="1.7" stroke-linecap="round" ' +
            'stroke-linejoin="round"/><path d="M3 12.4h10" stroke="currentColor" ' +
            'stroke-width="1.7" stroke-linecap="round"/></svg>';
        }
        if (label) {
          label.textContent =
            t.phase === 'failed' ? T('task.retry', '重试')
              : t.phase === 'ready' ? T('task.install', '安装')
                : t.paused ? T('task.paused', '已暂停')
                  : t.phase === 'queued' ? T('task.queuedShort', '排队中')
                    : t.phase === 'downloading'
                      ? (t.kind === 'update' ? T('task.updating', '更新中')
                        : T('task.downloading', '下载中'))
                      : (T('phase.' + t.phase, PHASE_TEXT[t.phase]) || '处理中');
        }
        btn.classList.toggle('is-failed', t.phase === 'failed');
      } else if (label) {
        var g = gameOf(key) || {};
        var inst = !!g.installed;
        /* 文案按安装状态取：cta.text 是未安装时的（获取/下载），
           cta.launchText 是装好后的（开始游戏） */
        label.textContent = g.updateTo ? '立即更新' : inst
          ? ((g.cta && g.cta.launchText) || '开始游戏')
          : ((g.cta && g.cta.text) || '获取游戏');
        btn.setAttribute('data-action', inst && !g.updateTo ? 'launch' : 'download');
        var disc = btn.querySelector('.disc');
        if (disc) disc.innerHTML = inst && !g.updateTo ? CONTROL_PLAY_SVG : '<i class="arrow"></i>';
        btn.classList.remove('is-busy', 'is-paused', 'entering');
      }
    }

    /* 副行：下载中显示 容量 + 「剩余时间/速度」交替闪烁；没任务时还原成定位链接 */
    var ctaSub = document.querySelector('.cta-sub');
    var ckey = btn && btn.getAttribute('data-game');
    var tk = ckey ? tasks[ckey] : null;
    if (ctaSub) {
      if (tk && tk.phase === 'downloading') {
        var left = tk.speed > 0 ? (tk.total - tk.done) / tk.speed : 0;
        ctaSub.innerHTML =
          '<span>' + gb(tk.done) + ' / ' + gb(tk.total) + '</span>' +
          '<span class="altswap">' +
          '<span class="t">剩余 ' + mmss(left) + '</span>' +
          '<span class="s">' + mbps(tk.speed) + '</span></span>';
      } else {
        /* 任务结束/取消后要把下载读数清掉，否则会一直挂着上一次的速度 */
        var want = subRowFor(ckey);
        if (ctaSub.innerHTML !== want) ctaSub.innerHTML = want;
      }
    }

    /* 任务列表：一条任务一个卡片，支持并发。失败态带重试。 */
    var box = document.getElementById('tasks');
    if (box) {
      if (!keys.length) {
        box.classList.add('hide');
        box.innerHTML = '';
      } else {
        box.classList.remove('hide');
        box.innerHTML = '';
        keys.forEach(function (k) {
          var t = tasks[k];
          var g = gameOf(k) || {};
          var node = el('div', 'task' + (t.phase === 'failed' ? ' failed' : '') +
            (t.leaving ? ' leaving' : ''));
          var nm = (g.name || k);
          var head = t.kind === 'update' ? T('task.updating', '正在更新 ') : T('task.downloading', '正在下载 ');

          if (t.phase === 'failed') {
            node.innerHTML =
              '<span class="msg">' + esc(nm) + ' · ' + esc(t.error || '下载失败') + '</span>' +
              '<button class="mini retry" data-tip="重试">' +
              '<svg viewBox="0 0 16 16"><path d="M13 8a5 5 0 1 1-1.5-3.6" fill="none" ' +
              'stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg></button>' +
              '<button class="mini cancel" data-tip="移除">' + CONTROL_SVG.close + '</button>';
            node.querySelector('.retry').addEventListener('click', function () {
              taskRetry(k);
            });
          } else {
            node.innerHTML =
              '<span class="name">' + esc(head + nm) + '</span>' +
              '<span class="pct">' + Math.round(t.percent) + '%</span>' +
              '<button class="mini pp" data-tip="' +
              (t.paused ? '继续' : '暂停') + '">' +
              (t.paused
                ? '<svg viewBox="0 0 16 16"><path d="M4 2.5l10 5.5-10 5.5z"/></svg>'
                : '<svg viewBox="0 0 16 16"><rect x="3" y="2.5" width="4" height="11" rx="1"/>' +
                  '<rect x="9" y="2.5" width="4" height="11" rx="1"/></svg>') +
              '</button>' +
              '<button class="mini cancel" data-tip="取消">' + CONTROL_SVG.close + '</button>' +
              '<i class="bar-mini" style="width:' + t.percent.toFixed(1) + '%"></i>';
            node.querySelector('.pp').addEventListener('click', function () {
              if (t.paused) taskResume(k); else taskPause(k);
            });
          }
          node.querySelector('.cancel').addEventListener('click', function () {
            taskCancel(k);
          });
          box.appendChild(node);
        });
      }
    }

    /* 侧栏图标：蒙层 + 百分比 + 角标 */
    keys.forEach(function (k) {
      var t = tasks[k];
      var icon = document.querySelector('.game-icon[data-side="' + k + '"]');
      if (!icon) return;
      var ov = icon.querySelector('.game-icon__overlay');
      if (ov) {
        ov.classList.add('visible');
        ov.innerHTML = '<span class="pct">' + Math.round(t.percent) + '%</span>';
      }
      if (!icon.querySelector('.game-icon__badge')) {
        var bd = document.createElement('i');
        bd.className = 'game-icon__badge';
        bd.innerHTML = '<svg viewBox="0 0 12 12"><path d="M6 1.5v6.5M3.2 5.4 6 8.2l2.8-2.8" ' +
          'fill="none" stroke="#16171a" stroke-width="1.7" stroke-linecap="round" ' +
          'stroke-linejoin="round"/></svg>';
        icon.appendChild(bd);
      }
    });

    /* 底部进度文字 */
    var prog = document.querySelector('.progress');
    if (prog && active) {
      prog.innerHTML = (active.kind === 'update' ? '正在更新 ' : '正在下载 ') +
        '<b>' + Math.round(active.percent) + '%</b>';
    }
  }

  /* ---- 检查更新 ---- */
  function checkUpdates() {
    setState.updateStatus = '正在检查…';
    renderSettings();
    /* 有宿主时按钮已经发了 settings.action，结果由宿主回推 updateResult；浏览器预览才模拟 */
    if (bridge.isWebView2) return;
    setTimeout(function () {
      var any = null;
      Object.keys(CFG.games || {}).forEach(function (k) {
        if (CFG.games[k] && CFG.games[k].updateTo) any = k;
      });
      if (any) {
        setState.updateStatus = '发现新版本 ' + (gameOf(any).updateTo || '');
      } else {
        setState.updateStatus = '已是最新版本';
      }
      renderSettings();
    }, 1600);
  }

  /* ================= 背景切换器（官方 #hyp-background-switcher）=================
     顶部居中、悬停显现，点圆点换背景（可含视频）。
     ============================================================================ */
  function renderBgSwitcher(game) {
    var host = document.getElementById('bgSwitch');
    var dots = document.getElementById('bgDots');
    if (!host || !dots) return;
    var list = (game && game.backgrounds) || [];
    if (list.length < 2) { host.style.display = 'none'; return; }
    host.style.display = '';
    dots.innerHTML = '';
    list.forEach(function (bg, i) {
      var b = el('button', i === state.bgIndex ? 'on' : '');
      b.setAttribute('data-tip', bg.name || ('背景 ' + (i + 1)));
      b.setAttribute('data-tip-place', 'bottom');
      if (bg.art) b.style.backgroundImage = 'url("' + bg.art + '")';
      b.addEventListener('click', function (e) {
        e.stopPropagation();
        state.bgIndex = i;
        Array.prototype.forEach.call(dots.children, function (c, j) {
          c.classList.toggle('on', j === i);
        });
        applyBackground({
          background: bg.art || game.background,
          titleOverlay: bg.titleOverlay !== undefined ? bg.titleOverlay : game.titleOverlay,
          video: bg.video || null,
        }, 0, game.scrim === true, bg.vignette || game.vignette);
        toast(T('bg.switched', '已切换背景') + '：' + (bg.name || (i + 1)), 'success');
      });
      dots.appendChild(b);
    });
  }

  /* ================= 区服切换（官方 .hyp-server-switcher）================= */
  function renderServerSwitcher(game) {
    var list = (game && game.servers) || [];
    var old = document.getElementById('srvHost');
    if (old) old.remove();
    if (!list.length) return;
    var host = el('div', 'srv-host');
    host.id = 'srvHost';
    var sw = el('div', 'srv-switch');
    var btn = el('button', 'srv-btn');
    var cur = list[Math.min(state.server, list.length - 1)];
    btn.innerHTML = '<span>' + esc(cur.name || cur.key || '') + '</span>';
    sw.appendChild(btn);
    host.appendChild(sw);
    document.getElementById('view').appendChild(host);

    btn.addEventListener('click', function (e) {
      e.stopPropagation();
      var html = '<div class="pop-sep"></div>' + list.map(function (s, i) {
        return '<button class="pop-item' + (i === state.server ? ' on' : '') +
          '" data-i="' + i + '">' + esc(s.name || s.key) +
          (s.tag ? ' <span class="uid">' + esc(s.tag) + '</span>' : '') + '</button>';
      }).join('');
      var pop = UI.openPopup(sw, html, { align: 'left', cls: 'srv-pop' });
      pop.addEventListener('click', function (ev) {
        var it = ev.target.closest('[data-i]');
        if (!it) return;
        state.server = +it.getAttribute('data-i');
        UI.closePopup();
        renderServerSwitcher(game);
        bridge.game.select(state.game);
        toast(T('server.switched', '已切换到 ') + (list[state.server].name || ''), 'success');
      });
    });
  }

  /* ================= 账号弹窗（官方 #hyp-account-popup）================= */
  function openAccountPopup(anchor) {
    var A = (CFG.account) || {};
    var html =
      '<div class="pop-head">' +
      '<img src="' + esc(A.avatar || 'assets/icons/logo-app.svg') + '" alt="">' +
      '<div><div class="nick">' + esc(A.nick || T('account.guest', '未登录')) + '</div>' +
      '<div class="uid">' + esc(A.uid || '—') + '</div></div></div>' +
      '<button class="pop-item" data-a="switch">' + esc(T('account.switch', '切换账号')) + '</button>' +
      '<button class="pop-item" data-a="logout">' + esc(T('account.logout', '退出登录')) + '</button>' +
      '<div class="pop-sep"></div>' +
      '<button class="pop-item" data-a="settings">' + esc(T('account.settings', '设置')) + '</button>' +
      '<button class="pop-item danger" data-a="quit">' + esc(T('account.quit', '退出启动器')) + '</button>';
    var pop = UI.openPopup(anchor, html, { align: 'left', cls: 'acct-pop' });
    pop.addEventListener('click', function (e) {
      var it = e.target.closest('[data-a]');
      if (!it) return;
      var a = it.getAttribute('data-a');
      UI.closePopup();
      if (a === 'settings') { openSettings(); return; }
      if (a === 'quit') { bridge.app.quit(); toast(T('account.quitSent', '已请求退出（需宿主接管）'), 'info'); return; }
      if (a === 'switch') { toast(T('account.switchSent', '请在宿主里完成账号切换'), 'info'); return; }
      if (a === 'logout') { toast(T('account.logoutSent', '已请求退出登录'), 'info'); return; }
    });
  }

  /* ================= 游戏右键菜单：启动 / 定位 / 校验 / 卸载 ================= */
  function openGameMenu(anchor, key) {
    var g = gameOf(key) || {};
    var html =
      '<div class="pop-head"><img src="' + esc(g.icon || '') + '" alt="">' +
      '<div><div class="nick">' + esc(g.name || key) + '</div>' +
      '<div class="uid">整合包 ' + esc(g.version || '—') + '</div></div></div>';
    if (g.installed) {
      html += (g.updateTo ? '<button class="pop-item" data-a="download">立即更新</button>' : '<button class="pop-item" data-a="launch">' + esc(T('menu.launch', '启动游戏')) + '</button>') +
        '<button class="pop-item" data-a="locate">' + esc(T('menu.locate', '打开安装目录')) + '</button>' +
        '<button class="pop-item" data-a="verify">' + esc(T('menu.verify', '校验文件完整性')) + '</button>';
    } else {
      html += '<button class="pop-item" data-a="download">' + esc(T('menu.download', '下载')) + '</button>';
    }
    var pop = UI.openPopup(anchor, html, { align: 'left', cls: 'game-pop' });
    pop.addEventListener('click', function (e) {
      var it = e.target.closest('[data-a]');
      if (!it) return;
      var a = it.getAttribute('data-a');
      UI.closePopup();
      if (a === 'launch') { bridge.game.launch(key); toast(T('menu.launched', '已请求启动'), 'success'); }
      else if (a === 'locate') bridge.game.locate(key);
      else if (a === 'verify') { toast(T('menu.verifying', '开始校验文件…'), 'info'); bridge.settings.action('verify'); }
      else if (a === 'download') taskStart(key, g.updateTo ? 'update' : 'install');
    });
  }

  /* ================= Hub 骨架屏 =================
     官方用 .hyp-el-skeleton；这里首屏先出骨架，避免卡片图未加载时闪一下。
     ============================================ */
  function hubSkeletonHtml() {
    var n = ((CFG.hub && CFG.hub.entries) || []).length || 3;
    var out = '<div class="hub-row hub-sk">';
    for (var i = 0; i < n; i++) {
      out += '<div class="hub-item"><div class="sk sk-card"><i></i></div></div>';
    }
    return out + '</div>';
  }

  function init() {
    /* 舞台模式：视口大于 1280x720 时把画布居中（浏览器 kiosk 预览无边框观感用） */
    if (/[?&]stage=1\b/.test(location.search)) {
      document.documentElement.classList.add('stage');
    }

    /* ---- 宿主 -> 页面 ----
       宿主可以用这些消息驱动界面，页面就不再自己模拟：
         { type:'game', action:'progress',    payload:{ game, percent, speed, phase, kind } }
         { type:'game', action:'status',      payload:{ game, installed, version, updateTo } }
         { type:'app',  action:'hostTakeover',payload:{ download:true } }   // 关掉模拟
         { type:'app',  action:'updateResult',payload:{ latest:true, version } }
         { type:'app',  action:'toast',       payload:{ text, kind, ms, button, then } }
         { type:'settings', action:'values',  payload:{ key: value, ... } }
         { type:'game', action:'news',        payload:{ game, tab, items:[{ title, date, href }] } }
       progress 的 phase 为 finished / failed（附 error）时直接收尾。
    ---- */
    bridge.onMessage(function (msg) {
      if (!msg || typeof msg !== 'object') return;
      var p = msg.payload || {};

      if (msg.type === 'app' && msg.action === 'releaseHistory') {
        showReleaseHistory(p);
        return;
      }

      if (msg.type === 'app' && msg.action === 'hostTakeover') {
        if (p.download != null) simulate = !p.download;
        return;
      }

      if (msg.type === 'game' && msg.action === 'progress' && p.game) {
        var t = tasks[p.game];
        if (!t) {
          t = tasks[p.game] = {
            key: p.game, kind: p.kind || 'install', phase: 'downloading',
            total: gameSize(p.game), done: 0, percent: 0, speed: 0, paused: false,
          };
          ensureTimer();
        }
        if (p.total) t.total = p.total;
        if (p.percent != null) {
          t.percent = Math.max(0, Math.min(100, p.percent));
          t.done = t.percent / 100 * t.total;
        }
        if (p.speed != null) t.speed = p.speed;
        if (p.paused != null) t.paused = !!p.paused;
        if (p.phase === 'finished') { taskFinish(p.game); return; }
        if (p.phase === 'failed') { taskFail(p.game, p.error); return; }
        if (p.phase) t.phase = p.phase;
        if (t.percent >= 100 && t.phase === 'downloading') t.phase = 'verifying';
        taskRender();
        return;
      }

      /* 联机大厅的房间列表（或查询失败的原因） */
      if (msg.type === 'lobby' && msg.action === 'rooms') {
        lobbyState.loading = false;
        lobbyState.error = p.error || '';
        lobbyState.rooms = p.rooms || [];
        if (p.version) lobbyState.version = p.version;
        lobbyState.mine = p.mine || null;
        if (lobbyState.joining && !lobbyState.rooms.some(function (r) { return r.id === lobbyState.joining; })) lobbyState.joining = null;
        renderLobby();
        return;
      }

      /* 宿主推送设置页的值（启动模式、游戏目录、当前版本、更新状态等），宿主是这些值的来源 */
      if (msg.type === 'settings' && msg.action === 'values') {
        Object.keys(p).forEach(function (k) { setState[k] = p[k]; });
        if (document.getElementById('setDialog')) renderSettings();
        return;
      }

      /* 宿主替换某个资讯分页的条目（检查更新后把最新版本的更新说明放进「公告」） */
      if (msg.type === 'game' && msg.action === 'news' && p.game && p.tab) {
        var ng = gameOf(p.game);
        if (ng) {
          ng.news = ng.news || {};
          if (typeof p.notes === 'string') {
            var points = window.towerReleaseNotes.summary(p.notes);
            if (!points.length) points = ['查看本次更新内容'];
            ng.news[p.tab] = points.map(function (point, index) {
              return { title: point, date: index === 0 ? p.version || '' : '', action: 'releaseHistory' };
            });
          } else ng.news[p.tab] = p.items || [];
          if (state.route === 'launcher' && state.game === p.game) renderNews();
        }
        return;
      }

      /* 宿主的一次性提示（启动失败原因、需要重启启动器等） */
      if (msg.type === 'app' && msg.action === 'toast') {
        /* 带按钮的提示（如「立即重启」）：点按钮把 then 作为 settings.action 交回宿主 */
        if (p.button && UI) {
          UI.toast(String(p.text || ''), {
            type: p.kind || 'info', duration: p.ms == null ? 4200 : p.ms, action: String(p.button),
            onAction: function () { bridge.settings.action(p.then); },
          });
        } else {
          toast(String(p.text || ''), p.kind || 'info', p.ms == null ? 4200 : p.ms);
        }
        return;
      }

      if (msg.type === 'game' && msg.action === 'status' && p.game) {
        var g = gameOf(p.game);
        if (g) {
          if (p.installed != null) g.installed = !!p.installed;
          if (p.version) g.version = p.version;
          if (p.note != null) g.note = p.note;
          g.updateTo = p.updateTo || null;
          if (state.route === 'launcher' && state.game === p.game) renderLauncher(p.game);
        }
        return;
      }

      if (msg.type === 'app' && msg.action === 'updateResult') {
        setState.updateStatus = p.latest === false
          ? ('发现新版本 ' + (p.version || ''))
          : '已是最新版本';
        if (document.getElementById('setDialog')) renderSettings();
      }
    });

    renderWindowControls();
    renderSidebar();
    renderSocial();

    /* 可用性：键盘焦点圈 + 自定义 tooltip */
    initA11y();
    /* 设置持久化：启动时就要把存过的值读进 setState。
       否则「限速」「同时下载任务数」这类只在打开设置弹窗后才生效。 */
    initSetState();
    loadRuntime();
    applyLocalSettings();
    /* 视口比 1280x720 小时整体等比缩放，避免溢出 */
    fitCanvas();
    window.addEventListener('resize', fitCanvas);

    var logoLink = document.getElementById('sidebarLogo');
    if (logoLink) {
      logoLink.addEventListener('click', function (e) {
        e.preventDefault();
        /* 只有一个游戏：logo 回到主页（外壳原来是去 Hub 游戏选择页）。点空白处回 Hub 也去掉了 */
        if (gameOf(CFG.app.primaryGame)) {
          state.route = 'launcher';
          renderLauncher(CFG.app.primaryGame);
        } else {
          goHub();
        }
      });
    }

    /* 首屏：支持 ?page=hub 或 ?game=<key> 覆盖配置，便于预览与验证 */
    var q = {};
    (location.search || '').replace(/^\?/, '').split('&').forEach(function (kv) {
      if (!kv) return;
      var p = kv.split('=');
      q[decodeURIComponent(p[0])] = decodeURIComponent(p[1] || '');
    });

    var wantHub = q.page === 'hub';
    var wantGame = q.game || CFG.app.primaryGame;
    var hubIdx = -1;
    CFG.hub.entries.forEach(function (e, i) {
      if (e.key === wantGame) hubIdx = i;
    });
    if (hubIdx >= 0) state.hubIndex = hubIdx;

    if (wantHub || !wantGame || !gameOf(wantGame)) {
      state.route = 'hub';
      renderHub();
    } else {
      state.route = 'launcher';
      renderLauncher(wantGame);
    }

    bridge.window.lockSize(1280, 720);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

  /* 供宿主或调试调用 */
  window.hoyoApp = {
    goHub: goHub,
    openGame: function (key) {
      state.route = 'launcher';
      renderLauncher(key);
    },
    openModal: openModal,
    closeModal: closeModal,
    openSettings: openSettings,
    /* 下载/更新：暴露出来便于宿主与调试（也能手动推一个假进度看效果） */
    taskStart: taskStart,
    taskPause: taskPause,
    taskResume: taskResume,
    taskCancel: taskCancel,
    taskRetry: taskRetry,
    taskFail: taskFail,
    taskPreDownload: taskPreDownload,
    taskInstallNow: taskInstallNow,
    tasks: tasks,
    setSimulate: function (v) { simulate = !!v; },
    checkUpdates: checkUpdates,
    /* 设置：读写本地存储，方便宿主/调试核对 */
    loadStore: loadStore,
    applyLocalSettings: applyLocalSettings,
    /* 通用 UI */
    ui: UI,
    state: state,
  };
})();
