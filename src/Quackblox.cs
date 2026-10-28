// Quackblox (StarX Studio): lets several Roblox clients run at once.
// Roblox allows one client per PC by owning a named mutex. If this program owns it
// first, no client can, and each new one opens instead of closing the previous one.
// It touches nothing in Roblox: no injection, no file edits, only the mutex.
using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Threading;
using System.Windows.Forms;
using Microsoft.Win32;

static class Quackblox
{
    const string RobloxMutexName = "ROBLOX_singletonMutex";
    const string OwnMutexName = "StarX_Quackblox";
    const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    const string RunValue = "Quackblox";

    static Mutex robloxMutex;
    static NotifyIcon tray;
    static ToolStripMenuItem statusItem;
    static System.Windows.Forms.Timer retryTimer;
    static bool warnedBusy;

    [STAThread]
    static void Main(string[] args)
    {
        bool firstCopy;
        using (var own = new Mutex(true, OwnMutexName, out firstCopy))
        {
            if (!firstCopy) return;

            // --test N: grab the mutex, print the result and hold it for N seconds.
            if (args.Length > 0 && args[0] == "--test")
            {
                Console.WriteLine(TryGrab() ? "HOLDING" : "BUSY");
                Thread.Sleep(1000 * (args.Length > 1 ? int.Parse(args[1]) : 0));
                return;
            }

            Application.EnableVisualStyles();
            BuildTray();
            if (!TryGrab())
            {
                retryTimer = new System.Windows.Forms.Timer { Interval = 2000 };
                retryTimer.Tick += delegate { if (TryGrab()) retryTimer.Stop(); };
                retryTimer.Start();
            }
            Application.Run();
            tray.Visible = false;
        }
    }

    // True once this process owns the Roblox mutex.
    static bool TryGrab()
    {
        bool created;
        var m = new Mutex(true, RobloxMutexName, out created);
        if (created)
        {
            robloxMutex = m;
            SetStatus("Ativo: pode abrir quantos Roblox quiser", true);
            return true;
        }
        // A Roblox client opened before us owns it. It frees up when that client closes.
        m.Dispose();
        SetStatus("Feche o Roblox aberto para ativar", false);
        if (!warnedBusy && tray != null)
        {
            warnedBusy = true;
            tray.ShowBalloonTip(5000, "Quackblox",
                "Já tem um Roblox aberto. Feche ele uma vez e o multi ativa sozinho.", ToolTipIcon.Info);
        }
        return false;
    }

    static void SetStatus(string text, bool active)
    {
        if (tray == null) return;
        statusItem.Text = text;
        tray.Text = "Quackblox: " + (active ? "ativo" : "aguardando");
        tray.Icon = active ? SystemIcons.Shield : SystemIcons.Warning;
    }

    static void BuildTray()
    {
        var menu = new ContextMenuStrip();
        statusItem = new ToolStripMenuItem("...") { Enabled = false };
        menu.Items.Add(statusItem);
        menu.Items.Add(new ToolStripSeparator());

        menu.Items.Add("Abrir Roblox", null, delegate { OpenRoblox(); });

        var startup = new ToolStripMenuItem("Iniciar com o Windows") { Checked = IsStartupOn() };
        startup.Click += delegate { SetStartup(!startup.Checked); startup.Checked = IsStartupOn(); };
        menu.Items.Add(startup);

        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add("Sair", null, delegate { Application.Exit(); });

        tray = new NotifyIcon { ContextMenuStrip = menu, Visible = true, Icon = SystemIcons.Warning };
        tray.DoubleClick += delegate { OpenRoblox(); };
    }

    // The player registered for roblox-player:// links, the same one the site launches.
    static string PlayerPath()
    {
        using (var k = Registry.ClassesRoot.OpenSubKey(@"roblox-player\shell\open\command"))
        {
            var cmd = k == null ? null : k.GetValue(null) as string;
            if (string.IsNullOrEmpty(cmd)) return null;
            cmd = cmd.Trim();
            string path = cmd.StartsWith("\"") ? cmd.Substring(1, cmd.IndexOf('"', 1) - 1) : cmd.Split(' ')[0];
            return File.Exists(path) ? path : null;
        }
    }

    static void OpenRoblox()
    {
        var path = PlayerPath();
        if (path == null)
        {
            MessageBox.Show("Não achei o Roblox instalado.", "Quackblox");
            return;
        }
        Process.Start(new ProcessStartInfo(path) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(path) });
    }

    static bool IsStartupOn()
    {
        using (var k = Registry.CurrentUser.OpenSubKey(RunKey))
            return k != null && k.GetValue(RunValue) != null;
    }

    static void SetStartup(bool on)
    {
        using (var k = Registry.CurrentUser.OpenSubKey(RunKey, true))
        {
            if (on) k.SetValue(RunValue, "\"" + Application.ExecutablePath + "\"");
            else k.DeleteValue(RunValue, false);
        }
    }
}
