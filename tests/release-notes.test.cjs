const test = require('node:test');
const assert = require('node:assert/strict');
const notes = require('../launcher/ui/js/release-notes.js');

test('renders player changes as Markdown and removes developer sections', () => {
  const html = notes.render('## 更新内容\n- 修复**商店**\n- 支持 `拼音`\n\n## 更新须知\n- Steam 69 / TCP 46\n### 迁移\n- 开发提示\n## 验证\n- 433 项测试\n## 修复\n1. 加载更快');
  assert.match(html, /<h2>更新内容<\/h2>/);
  assert.match(html, /<ul>/);
  assert.match(html, /<strong>商店<\/strong>/);
  assert.match(html, /<code>拼音<\/code>/);
  assert.match(html, /<ol>/);
  for (const text of ['更新须知', 'Steam 69', '迁移', '开发提示', '433 项测试']) assert.ok(!html.includes(text));
});

test('preserves patch notes containing verification words and fenced samples', () => {
  const html = notes.render('## 修复\n- 修复验证失败\n\n```text\n## 验证\n```\n\n> 注意备份\n\n[发布页](https://example.com)');
  assert.match(html, /修复验证失败/);
  assert.match(html, /<pre><code/);
  assert.match(html, /## 验证/);
  assert.match(html, /<blockquote>/);
  assert.match(html, /href="https:\/\/example.com"/);
});

test('does not execute HTML, unsafe links, or remote images', () => {
  const html = notes.render('<script>alert(1)</script>\n\n[坏链接](javascript:alert(1))\n\n![图](https://example.com/tracker.png)\n\n[本地](file:///C:/secret)');
  assert.ok(!html.includes('<script>'));
  assert.ok(!html.includes('<img'));
  assert.ok(!html.includes('href="javascript:'));
  assert.ok(!html.includes('href="file:'));
});

test('hides build labels while retaining release versions', () => {
  assert.equal(notes.title('黄金树爬塔 1.6.1（构建 v106）'), '黄金树爬塔 1.6.1');
  assert.equal(notes.title('黄金树爬塔 1.6.1 · v106'), '黄金树爬塔 1.6.1');
  assert.equal(notes.title('v106'), '历史版本');
  assert.equal(notes.title('v1.6.1'), 'v1.6.1');
  assert.equal(notes.title(''), '历史版本');
  const html = notes.render('整合包 1.6.1（构建 v106），启动器 1.4.0。\n\n## 更新内容\n- 改善体验');
  assert.match(html, /1\.6\.1/);
  assert.match(html, /1\.4\.0/);
  assert.ok(!/构建|v106/.test(html));
});

test('homepage shows four update points without Markdown headings or version preamble', () => {
  const summary = notes.summary('# 黄金树爬塔 1.6.1\n\n整合包 1.6.1，启动器保持 1.3.2。\n\n## 更新内容\n- 修复**武器质变**。\n- 支持 `qhs` 搜索。\n- 优化[联机](https://example.com)。\n- 改善商店。\n- 第五条。\n## 验证\n- 433 项测试');
  assert.deepEqual(summary, ['修复武器质变。', '支持 qhs 搜索。', '优化联机。', '改善商店。']);
});

test('homepage ignores developer sections and nested implementation lists', () => {
  assert.deepEqual(notes.summary('## 验证\n- 80 tests\n## 更新内容\n- 改善联机\n  - 内部实现\n- 修复商店'), ['改善联机', '修复商店']);
  assert.deepEqual(notes.summary('## 验证\n- 80 tests'), []);
});
