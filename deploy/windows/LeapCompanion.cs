using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Windows.Forms;
using Microsoft.Win32;

internal static class LeapCompanion
{
    private static readonly string InstallDir = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "MeshGraph");
    private static readonly string Launcher = Path.Combine(InstallDir, "MeshGraphLeap.exe");
    private static readonly string Bridge = Path.Combine(InstallDir, "hyperion_bridge.exe");

    [STAThread]
    private static void Main(string[] args)
    {
        try
        {
            if (args.Length == 2 && args[0] == "--open") Activate(args[1]);
            else Install();
        }
        catch (Exception error)
        {
            MessageBox.Show(error.Message, "MeshGraph Leap Motion", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
    }

    private static void Install()
    {
        if (MessageBox.Show(
            "Install the MeshGraph Leap Motion companion for your Windows account? " +
            "After installation, the app's Leap Motion button starts the bridge in the background. " +
            "Ultraleap Hyperion and a connected sensor are also required.",
            "MeshGraph Leap Motion", MessageBoxButtons.OKCancel, MessageBoxIcon.Question) != DialogResult.OK) return;
        Directory.CreateDirectory(InstallDir);
        string source = Assembly.GetExecutingAssembly().Location;
        if (!String.Equals(source, Launcher, StringComparison.OrdinalIgnoreCase)) File.Copy(source, Launcher, true);
        var existing = Process.GetProcessesByName("hyperion_bridge");
        bool installedBridgeRunning = existing.Any(p => {
            try { return String.Equals(p.MainModule.FileName, Bridge, StringComparison.OrdinalIgnoreCase); }
            catch { return false; }
        });
        if (installedBridgeRunning) throw new Exception("Close the installed hyperion_bridge process in Task Manager before updating the companion.");
        using (Stream resource = Assembly.GetExecutingAssembly().GetManifestResourceStream("hyperion_bridge.exe"))
        using (FileStream output = File.Create(Bridge)) resource.CopyTo(output);
        using (RegistryKey protocol = Registry.CurrentUser.CreateSubKey(@"Software\Classes\meshgraph-leap"))
        {
            protocol.SetValue("", "URL:MeshGraph Leap Motion");
            protocol.SetValue("URL Protocol", "");
            using (RegistryKey command = protocol.CreateSubKey(@"shell\open\command"))
                command.SetValue("", "\"" + Launcher + "\" --open \"%1\"");
        }
        MessageBox.Show("Installed. Return to MeshGraph and click Leap Motion. Allow the browser to open MeshGraphLeap when prompted. No terminal is needed.",
            "MeshGraph Leap Motion", MessageBoxButtons.OK, MessageBoxIcon.Information);
    }

    private static void Activate(string value)
    {
        Uri launch;
        if (!Uri.TryCreate(value, UriKind.Absolute, out launch) || launch.Scheme != "meshgraph-leap" || launch.Host != "start")
            throw new Exception("Invalid MeshGraph activation link.");
        string originValue = launch.Query.TrimStart('?').Split('&')
            .Where(p => p.StartsWith("origin=", StringComparison.Ordinal)).Select(p => Uri.UnescapeDataString(p.Substring(7))).FirstOrDefault();
        Uri origin;
        if (!Uri.TryCreate(originValue, UriKind.Absolute, out origin) ||
            !(origin.Scheme == "https" || (origin.Scheme == "http" && origin.IsLoopback)) ||
            !String.IsNullOrEmpty(origin.UserInfo) || origin.GetLeftPart(UriPartial.Authority) != originValue)
            throw new Exception("Activation must include an HTTPS application origin (or localhost for development).");
        string originsFile = Path.Combine(InstallDir, "allowed-origins.txt");
        string[] allowed = File.Exists(originsFile) ? File.ReadAllLines(originsFile) : new string[0];
        if (!allowed.Contains(originValue))
        {
            if (MessageBox.Show("Allow " + originValue + " to access your Leap Motion hand tracking while you enable IR input?",
                "MeshGraph Leap Motion", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
            File.AppendAllText(originsFile, originValue + Environment.NewLine);
        }
        string runtime = Environment.GetEnvironmentVariable("HYPERION_LEAPC_PATH") ?? @"C:\Program Files\Ultraleap\LeapSDK\lib\x64\LeapC.dll";
        if (!File.Exists(runtime)) throw new Exception("Install Ultraleap Hyperion first. LeapC.dll was not found at " + runtime);
        Process.Start(new ProcessStartInfo(Bridge) {
            UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = InstallDir,
            WindowStyle = ProcessWindowStyle.Hidden
        });
    }
}
