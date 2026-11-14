using System;

namespace Tower.Launcher
{
    static class LauncherWindowSize
    {
        public const string Default = "1280 × 720";
        public static readonly string[] Choices = { "960 × 540", Default, "1600 × 900", "1920 × 1080" };
        public static string Normalize(string? value) => Array.IndexOf(Choices, value) >= 0 ? value! : Default;

        public static (int Width, int Height) Fit(string? value, int dpi, int availableWidth, int availableHeight)
        {
            var index = Array.IndexOf(Choices, Normalize(value));
            var width = new[] { 960, 1280, 1600, 1920 }[index];
            var scale = Math.Min(Math.Max(96, dpi) / 96.0,
                Math.Min(Math.Max(1, availableWidth) / (double)width, Math.Max(1, availableHeight) / (width * 9.0 / 16.0)));
            return (Math.Max(1, (int)Math.Floor(width * scale)), Math.Max(1, (int)Math.Floor(width * 9.0 / 16.0 * scale)));
        }
    }
}
