(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory(require('./vendor/markdown-it.min.js'));
  else root.towerReleaseNotes = factory(root.markdownit);
})(typeof window === 'undefined' ? globalThis : window, function (markdownit) {
  'use strict';
  var md = markdownit({ html: false, linkify: false, breaks: false }).disable('image');
  md.validateLink = function (url) { return /^https?:\/\//i.test(url); };

  function playerText(text) {
    return String(text || '').replace(/[（(]\s*(?:内部\s*)?(?:构建(?:号)?|build)\s*[:：]?\s*v?\d+\s*[)）]/gi, '')
      .replace(/(?:内部\s*)?构建(?:号)?\s*[:：]?\s*v?\d+\b/gi, '');
  }

  function title(text) {
    return playerText(text).replace(/(?:[·|｜/-]\s*)?\bv\d+\b(?!\.)/gi, '').trim() || '历史版本';
  }

  function playerTokens(text) {
    var tokens = md.parse(playerText(text), {});
    var visible = [], hiddenLevel = 0;
    tokens.forEach(function (token, index) {
      if (token.type === 'heading_open' && token.level === 0) {
        var level = +token.tag.slice(1);
        if (hiddenLevel && level <= hiddenLevel) hiddenLevel = 0;
        var title = (tokens[index + 1].children || []).map(function (child) { return child.content; }).join('').trim();
        if (/^(更新须知|验证)(\s|[：:（(]|$)/.test(title)) hiddenLevel = level;
      }
      if (!hiddenLevel) visible.push(token);
    });
    return visible;
  }

  function render(text) {
    return md.renderer.render(playerTokens(text), md.options, {});
  }

  function summary(text) {
    var rows = [], depth = 0, current = [];
    playerTokens(text).forEach(function (token) {
      if (token.type === 'list_item_open') {
        if (++depth === 1) current = [];
      } else if (token.type === 'list_item_close') {
        if (--depth === 0 && current.length) rows.push(current.join(' ').trim());
      } else if (token.type === 'inline' && depth === 1) {
        current.push((token.children || []).map(function (child) {
          return child.type === 'text' || child.type === 'code_inline' ? child.content
            : child.type === 'softbreak' || child.type === 'hardbreak' ? ' ' : '';
        }).join(''));
      }
    });
    return rows.filter(Boolean).slice(0, 4);
  }
  return { render: render, title: title, summary: summary };
});
