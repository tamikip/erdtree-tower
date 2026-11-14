using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Threading;
using System.Threading.Tasks;

namespace Tower.Update
{
    /// <summary>更新数据从哪里来：最新清单，以及某个版本数据包里的一段字节。</summary>
    public interface IReleaseSource
    {
        Task<ReleaseManifest> LatestAsync(CancellationToken cancel);
        Task<byte[]> RangeAsync(string tag, string asset, long offset, long length, CancellationToken cancel);
    }

    public interface IVersionedReleaseSource : IReleaseSource
    {
        Task<ReleaseManifest> VersionAsync(string tag, CancellationToken cancel);
    }

    /// <summary>
    /// GitHub Release：最新清单走 releases/latest/download（不经过 API，没有次数限制），
    /// 数据包用 Range 请求只取需要的那几段（GitHub 的下载地址会回 206）。
    /// baseUrl 也可以换成镜像，只要目录结构相同：{base}/latest/download/{asset} 与 {base}/download/{tag}/{asset}。
    /// 跳转由这里自己跟（每一跳都带上 Range，不依赖 HttpClient 自动跳转时是否保留请求头；传入的 HttpClient 应关闭 AllowAutoRedirect），
    /// 且不允许从 https 跳到 http。
    /// </summary>
    public sealed class HttpReleaseSource : IVersionedReleaseSource
    {
        readonly HttpClient http;
        readonly string baseUrl;

        public HttpReleaseSource(HttpClient http, string baseUrl)
        {
            this.http = http;
            this.baseUrl = baseUrl.TrimEnd('/');
        }

        public static HttpReleaseSource GitHub(HttpClient http, string owner, string repo) => new HttpReleaseSource(http, $"https://github.com/{owner}/{repo}/releases");

        public async Task<ReleaseManifest> LatestAsync(CancellationToken cancel)
        {
            using var response = await SendAsync($"{baseUrl}/latest/download/{ReleaseManifest.FileName}", null, cancel).ConfigureAwait(false);
            EnsureSuccess(response, "发布页上的更新清单");
            return ReleaseManifest.Parse(await response.Content.ReadAsStringAsync().ConfigureAwait(false));
        }

        public async Task<byte[]> RangeAsync(string tag, string asset, long offset, long length, CancellationToken cancel)
        {
            if (length == 0) return Array.Empty<byte>();
            var range = new RangeHeaderValue(offset, offset + length - 1);
            using var response = await SendAsync($"{baseUrl}/download/{Uri.EscapeDataString(tag)}/{Uri.EscapeDataString(asset)}", range, cancel).ConfigureAwait(false);
            EnsureSuccess(response, $"更新数据 {tag}/{asset}");
            // 服务器不支持分段时会回 200 和整个文件：拒绝，避免把整包当成一段数据
            if (response.StatusCode != System.Net.HttpStatusCode.PartialContent) throw new IOException("下载服务器不支持分段下载");
            var data = await response.Content.ReadAsByteArrayAsync().ConfigureAwait(false);
            if (data.LongLength != length) throw new IOException($"下载的数据长度不对：{data.LongLength}/{length}");
            return data;
        }

        public async Task<ReleaseManifest> VersionAsync(string tag, CancellationToken cancel)
        {
            VersionSwitch.Build(tag);
            using var response = await SendAsync($"{baseUrl}/download/{tag}/{ReleaseManifest.FileName}", null, cancel).ConfigureAwait(false);
            EnsureSuccess(response, "所选版本的更新清单");
            return VersionSwitch.Check(tag, ReleaseManifest.Parse(await response.Content.ReadAsStringAsync().ConfigureAwait(false)));
        }

        static void EnsureSuccess(HttpResponseMessage response, string what)
        {
            if (response.IsSuccessStatusCode) return;
            var code = (int)response.StatusCode;
            throw new IOException(code == 404 ? $"{what}不存在（HTTP 404）" : $"{what}下载失败（HTTP {code}）");
        }

        const int MaxRedirects = 8;

        async Task<HttpResponseMessage> SendAsync(string url, RangeHeaderValue? range, CancellationToken cancel)
        {
            var uri = new Uri(url);
            for (var hop = 0; ; hop++)
            {
                HttpResponseMessage response;
                using (var request = new HttpRequestMessage(HttpMethod.Get, uri))
                {
                    if (range != null) request.Headers.Range = range;
                    response = await http.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancel).ConfigureAwait(false);
                }
                var code = (int)response.StatusCode;
                var location = response.Headers.Location;
                if (code < 300 || code > 399 || location == null) return response;
                response.Dispose();
                if (hop == MaxRedirects) throw new IOException("下载地址跳转次数过多");
                var next = location.IsAbsoluteUri ? location : new Uri(uri, location);
                if (uri.Scheme == Uri.UriSchemeHttps && next.Scheme != Uri.UriSchemeHttps) throw new IOException($"拒绝从 https 跳转到不安全的地址：{next.Host}");
                uri = next;
            }
        }
    }

    /// <summary>本地目录（测试或离线更新包）：{root}/{tag}/{asset}，最新清单在 {root}/latest/tower-release.json。</summary>
    public sealed class DirectoryReleaseSource : IVersionedReleaseSource
    {
        readonly string root;
        public DirectoryReleaseSource(string root) { this.root = root; }

        public async Task<ReleaseManifest> VersionAsync(string tag, CancellationToken cancel)
        {
            VersionSwitch.Build(tag); cancel.ThrowIfCancellationRequested();
            return await Task.FromResult(VersionSwitch.Check(tag, ReleaseManifest.Parse(File.ReadAllText(Path.Combine(root, tag, ReleaseManifest.FileName)))));
        }

        public Task<ReleaseManifest> LatestAsync(CancellationToken cancel) =>
            Task.FromResult(ReleaseManifest.Parse(File.ReadAllText(Path.Combine(root, "latest", ReleaseManifest.FileName))));

        public Task<byte[]> RangeAsync(string tag, string asset, long offset, long length, CancellationToken cancel)
        {
            using var stream = File.OpenRead(Path.Combine(root, tag, asset));
            if (offset + length > stream.Length) throw new IOException("数据包长度不足");
            stream.Position = offset;
            var data = new byte[length];
            var read = 0;
            while (read < length)
            {
                var n = stream.Read(data, read, (int)Math.Min(int.MaxValue, length - read));
                if (n == 0) throw new IOException("数据包读取中断");
                read += n;
            }
            return Task.FromResult(data);
        }
    }

    /// <summary>
    /// 多条下载线路（如国内镜像 + GitHub），按偏好顺序排列。
    /// 最新清单：同时问每条线路，第一份到手后最多再等 Grace，取版本最高的一份（同版本按偏好顺序），
    /// 这样镜像还没上传新版本时仍能从别的线路拿到新版本；一直连不上的线路不会拖慢检查。
    /// 数据分段：先走选中的线路，失败就换下一条，失败过的线路排到最后。
    /// </summary>
    public sealed class MultiSource : IVersionedReleaseSource
    {
        readonly IReadOnlyList<KeyValuePair<string, IReleaseSource>> lines;
        readonly object gate = new object();
        List<int> order;

        public TimeSpan Grace { get; set; } = TimeSpan.FromSeconds(3);
        /// <summary>上一次最新清单来自哪条线路</summary>
        public string? Chosen { get; private set; }

        public MultiSource(IEnumerable<KeyValuePair<string, IReleaseSource>> lines)
        {
            this.lines = lines.ToList();
            if (this.lines.Count == 0) throw new ArgumentException("至少需要一条线路");
            order = Enumerable.Range(0, this.lines.Count).ToList();
        }

        public async Task<ReleaseManifest> LatestAsync(CancellationToken cancel)
        {
            using var stragglers = CancellationTokenSource.CreateLinkedTokenSource(cancel);
            var tasks = lines.Select(l => l.Value.LatestAsync(stragglers.Token)).ToList();
            var pending = new List<Task>(tasks);
            Task? deadline = null;
            while (pending.Count > 0)
            {
                var next = await Task.WhenAny(deadline == null ? pending : pending.Append(deadline)).ConfigureAwait(false);
                if (next == deadline) break;
                pending.Remove(next);
                if (next.Status == TaskStatus.RanToCompletion && deadline == null) deadline = Task.Delay(Grace, cancel);
            }
            stragglers.Cancel();
            foreach (var task in pending) _ = task.ContinueWith(t => t.Exception, TaskContinuationOptions.OnlyOnFaulted);
            cancel.ThrowIfCancellationRequested();

            var answered = Enumerable.Range(0, lines.Count).Where(i => tasks[i].Status == TaskStatus.RanToCompletion).ToList();
            if (answered.Count == 0)
            {
                if (lines.Count == 1) await tasks[0].ConfigureAwait(false);
                throw new IOException("所有下载线路都没拿到更新清单：" + string.Join("；", Enumerable.Range(0, lines.Count).Select(i => $"{lines[i].Key}：{Reason(tasks[i])}")));
            }
            var best = answered.Max(i => tasks[i].Result.Version);
            var same = answered.Where(i => tasks[i].Result.Version == best).ToList();
            // 分段下载的顺序：有这个版本的线路（偏好顺序）在前，没来得及回答的线路兜底；版本落后的线路没有新数据包，不用
            var unknown = Enumerable.Range(0, lines.Count).Where(i => !tasks[i].IsCompleted || tasks[i].IsCanceled).ToList();
            lock (gate) order = same.Concat(unknown).ToList();
            Chosen = lines[same[0]].Key;
            return tasks[same[0]].Result;
        }

        static string Reason(Task task) => task.IsCanceled ? "超时" : task.Exception?.InnerException?.Message ?? "未知错误";

        public async Task<ReleaseManifest> VersionAsync(string tag, CancellationToken cancel)
        {
            VersionSwitch.Build(tag);
            Exception? last = null;
            foreach (var i in Enumerable.Range(0, lines.Count))
            {
                if (!(lines[i].Value is IVersionedReleaseSource versioned)) continue;
                using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancel);
                timeout.CancelAfter(TimeSpan.FromSeconds(20));
                try {
                    var result = await versioned.VersionAsync(tag, timeout.Token).ConfigureAwait(false);
                    lock (gate) order = new[] { i }.Concat(Enumerable.Range(0, lines.Count).Where(n => n != i)).ToList();
                    Chosen = lines[i].Key;
                    return result;
                } catch (Exception e) when (!cancel.IsCancellationRequested && (e is IOException || e is HttpRequestException || e is OperationCanceledException)) { last = e; }
            }
            cancel.ThrowIfCancellationRequested();
            throw last ?? new IOException("该历史版本没有可用的更新清单");
        }

        public async Task<byte[]> RangeAsync(string tag, string asset, long offset, long length, CancellationToken cancel)
        {
            List<int> attempt;
            lock (gate) attempt = order.ToList();
            Exception? last = null;
            foreach (var i in attempt)
            {
                try { return await lines[i].Value.RangeAsync(tag, asset, offset, length, cancel).ConfigureAwait(false); }
                catch (Exception e) when (!cancel.IsCancellationRequested && (e is IOException || e is HttpRequestException || e is OperationCanceledException))
                {
                    last = e;
                    lock (gate) { if (order.Remove(i)) order.Add(i); }
                }
            }
            throw last ?? new IOException("没有可用的下载线路");
        }
    }
}
