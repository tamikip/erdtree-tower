using System.Text.RegularExpressions;

namespace Tower.Update
{
    public static class Versions
    {
        public static bool Valid(string value) => Regex.IsMatch(value, @"\A(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\z");
        public static string Label(int build, string? display) => !string.IsNullOrEmpty(display) && Valid(display!) ? display! : "v" + build;
        public static string PlayerLabel(string? display) => !string.IsNullOrEmpty(display) && Valid(display!) ? display! : "历史版本";
    }
}
