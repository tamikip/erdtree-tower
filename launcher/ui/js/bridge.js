/* ============================================================
   与宿主 exe 的通信桥。

   WebView2 环境下用 window.chrome.webview.postMessage 发送消息，
   宿主侧用 CoreWebView2.WebMessageReceived 接收。
   在普通浏览器里打开时全部降级为控制台日志，方便单独预览调试。

   消息格式统一为 { type, action, payload }
   ============================================================ */

(function () {
  'use strict';

  var isWebView2 =
    typeof window.chrome !== 'undefined' &&
    window.chrome !== null &&
    typeof window.chrome.webview !== 'undefined';

  var listeners = [];

  function post(type, action, payload) {
    var msg = { type: type, action: action, payload: payload || null };

    if (isWebView2) {
      window.chrome.webview.postMessage(msg);
    } else {
      // 浏览器里预览时打印出来，便于对照宿主端应该实现什么
      console.log('[bridge]', msg);
    }
    return msg;
  }

  var bridge = {
    isWebView2: isWebView2,

    /* ---- 窗口控制 ---- */
    window: {
      minimize: function () {
        post('window', 'minimize');
      },
      maximize: function () {
        post('window', 'toggleMaximize');
      },
      close: function () {
        post('window', 'close');
      },
      /* 无边框窗口拖动：宿主收到后调用 ReleaseCapture + SendMessage(WM_NCLBUTTONDOWN) */
      startDrag: function () {
        post('window', 'startDrag');
      },
      /* 让宿主把窗口设为固定尺寸（本界面按 1280x720 设计） */
      lockSize: function (w, h) {
        post('window', 'lockSize', { width: w || 1280, height: h || 720 });
      },
    },

    /* ---- 游戏相关 ---- */
    game: {
      /* 通知宿主切换当前游戏（宿主可据此改窗口标题、记配置） */
      select: function (gameKey) {
        post('game', 'select', { game: gameKey });
      },
      /* 安装 / 更新 */
      download: function (gameKey) {
        post('game', 'download', { game: gameKey });
      },
      /* 启动已安装的游戏 */
      launch: function (gameKey) {
        post('game', 'launch', { game: gameKey });
      },
      /* 定位游戏安装目录 */
      locate: function (gameKey) {
        post('game', 'locate', { game: gameKey });
      },
      /* 宿主可以回推安装状态，界面会更新按钮 */
      queryStatus: function (gameKey) {
        post('game', 'queryStatus', { game: gameKey });
      },
      /* ---- 下载 / 更新控制 ----
         页面负责 UI 状态机（没宿主时会自己模拟进度），
         真正干活交给宿主；宿主可用 game.progress 回推真实进度覆盖模拟。 */
      pause: function (gameKey) {
        post('game', 'pause', { game: gameKey });
      },
      resume: function (gameKey) {
        post('game', 'resume', { game: gameKey });
      },
      cancel: function (gameKey) {
        post('game', 'cancel', { game: gameKey });
      },
      /* 下载完成后安装（有些实现会分两步） */
      install: function (gameKey) {
        post('game', 'install', { game: gameKey });
      },
      /* 主动检查该游戏是否有新版本 */
      checkUpdate: function (gameKey) {
        post('game', 'checkUpdate', { game: gameKey });
      },
      /* 预下载（下载完不自动装，等用户点） */
      preDownload: function (gameKey) {
        post('game', 'preDownload', { game: gameKey });
      },
      uninstall: function (gameKey) {
        post('game', 'uninstall', { game: gameKey });
      },
      /* 暂停时可选：把已下载的分片落盘 / 清理临时文件 */
      flush: function (gameKey) {
        post('game', 'flush', { game: gameKey });
      },
    },

    /* ---- 设置 ---- */
    /* 联机大厅（宿主 TowerHost 的 lobby.*）：列出房间、开房、加入；关掉对话框时断开启动器的 Steam 连接 */
    lobby: {
      list: function (query) { post('lobby', 'list', { query: query || '' }); },
      host: function (room) { post('lobby', 'host', room); },
      join: function (id, password) { post('lobby', 'join', { id: id, password: password || '' }); },
      quick: function () { post('lobby', 'quick'); },
      code: function (code) { post('lobby', 'code', { code: code }); },
      close: function () { post('lobby', 'close'); },
    },
    settings: {
      change: function (key, value) {
        post('settings', 'change', { key: key, value: value });
      },
      action: function (name, payload) {
        post('settings', 'action', { name: name, payload: payload || null });
      },
    },

    /* ---- 应用级 ---- */
    app: {
      openSettings: function () {
        post('app', 'openSettings');
      },
      checkUpdate: function () {
        post('app', 'checkUpdate');
      },
      openPath: function (p) {
        post('app', 'openPath', { path: p });
      },
      quit: function () {
        post('app', 'quit');
      },
      /* 页面希望知道宿主是否接管下载：宿主可回 { type:'app', action:'hostTakeover', payload:{download:true} } */
      queryHost: function () {
        post('app', 'queryHost');
      },
    },

    /* ---- 通用 ---- */
    openExternal: function (url) {
      post('shell', 'openExternal', { url: url });
    },
    openSettings: function () {
      post('app', 'openSettings');
    },
    /* 宿主 -> 页面：注册消息处理 */
    onMessage: function (fn) {
      listeners.push(fn);
    },
  };

  if (isWebView2) {
    window.chrome.webview.addEventListener('message', function (e) {
      var data = e.data;
      listeners.forEach(function (fn) {
        try {
          fn(data);
        } catch (err) {
          console.error('[bridge] listener error', err);
        }
      });
    });
  }

  /* 暴露给宿主/调试用 */
  window.__hoyoBridge = bridge;
  window.hoyoBridge = bridge;

  /* ---- 自动绑定 data-action ---- */
  document.addEventListener('click', function (e) {
    var el = e.target.closest('[data-action]');
    if (!el) return;
    var action = el.getAttribute('data-action');
    var game = el.getAttribute('data-game') || null;

    switch (action) {
      case 'settings':
        bridge.openSettings();
        break;
      case 'minimize':
        bridge.window.minimize();
        break;
      case 'maximize':
        bridge.window.maximize();
        break;
      case 'close':
        bridge.window.close();
        break;
      case 'external':
        bridge.openExternal(el.getAttribute('data-url') || '#');
        break;
      case 'download':
        bridge.game.download(game);
        break;
      case 'pause':
        bridge.game.pause(game);
        break;
      case 'resume':
        bridge.game.resume(game);
        break;
      case 'cancel':
        bridge.game.cancel(game);
        break;
      case 'launch':
        bridge.game.launch(game);
        break;
      case 'locate':
        bridge.game.locate(game);
        break;
      default:
        break;
    }
  });

  /* ---- 无边框窗口拖动 ---- */
  document.addEventListener('mousedown', function (e) {
    if (e.button !== 0) return;
    var dragZone = e.target.closest('[data-drag]');
    if (!dragZone) return;
    /* 点在按钮上不触发拖动 */
    if (e.target.closest('button, a, input')) return;
    bridge.window.startDrag();
  });
})();
