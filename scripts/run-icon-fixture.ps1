param([switch]$Launch)

$ErrorActionPreference = 'Stop'
$fixtureDir = Join-Path $env:TEMP 'pixel-hunter-icon-fixture'
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null
$sourcePath = Join-Path $fixtureDir 'IconSearchFixture.cs'
$manifestPath = Join-Path $fixtureDir 'fixture.manifest'
$exePath = Join-Path $fixtureDir 'icon-search-fixture.exe'

@'
using System;
using System.Drawing;
using System.Windows.Forms;

sealed class Fixture : Form {
    readonly Panel screen = new Panel();
    readonly Label count = new Label();
    readonly Timer timer = new Timer();
    readonly Bitmap icon = new Bitmap(44, 44);
    readonly Rectangle search = new Rectangle(240, 35, 370, 180);
    bool present = true;
    bool moving = true;
    int position = 0;
    int presses = 0;

    Fixture() {
        Text = "Icon Search Fixture";
        StartPosition = FormStartPosition.Manual;
        Location = new Point(50, 50);
        ClientSize = new Size(700, 450);
        AutoScaleMode = AutoScaleMode.None;
        FormBorderStyle = FormBorderStyle.FixedSingle;
        MaximizeBox = false;
        KeyPreview = true;
        BackColor = Color.White;
        screen.Location = new Point(0, 45);
        screen.Size = new Size(700, 270);
        screen.Paint += DrawScreen;
        Controls.Add(screen);
        var intro = new Label { Text = "Select the blue panel, then the colorful icon. Red pixel is separate.", Location = new Point(15, 15), AutoSize = true };
        Controls.Add(intro);
        AddButton("Pause movement", 20, delegate { moving = !moving; });
        AddButton("Move icon", 210, delegate { position = (position + 80) % 300; screen.Invalidate(); });
        AddButton("Toggle icon", 400, delegate { present = !present; screen.Invalidate(); });
        count.Location = new Point(20, 390);
        count.AutoSize = true;
        count.Text = "KeyQ presses: 0";
        Controls.Add(count);
        KeyDown += delegate(object sender, KeyEventArgs args) {
            if (args.KeyCode == Keys.Q) {
                count.Text = "KeyQ presses: " + (++presses);
                args.Handled = true;
            }
        };
        using (var g = Graphics.FromImage(icon)) {
            g.Clear(Color.FromArgb(30, 18, 65));
            using (var cyan = new SolidBrush(Color.FromArgb(0, 220, 220))) g.FillRectangle(cyan, 3, 3, 38, 38);
            using (var purple = new SolidBrush(Color.FromArgb(145, 25, 200))) g.FillEllipse(purple, 7, 6, 30, 32);
            using (var yellow = new SolidBrush(Color.FromArgb(255, 200, 10))) {
                g.FillRectangle(yellow, 11, 12, 7, 7);
                g.FillRectangle(yellow, 26, 12, 7, 7);
                g.FillRectangle(yellow, 15, 29, 15, 4);
            }
            g.DrawLine(Pens.White, 20, 20, 24, 25);
        }
        timer.Interval = 800;
        timer.Tick += delegate { if (moving) { position = (position + 37) % 300; screen.Invalidate(); } };
        timer.Start();
    }

    void AddButton(string text, int x, EventHandler handler) {
        var button = new Button { Text = text, Location = new Point(x, 335), Size = new Size(175, 36) };
        button.Click += handler;
        Controls.Add(button);
    }

    void DrawScreen(object sender, PaintEventArgs args) {
        var g = args.Graphics;
        g.Clear(Color.White);
        using (var background = new SolidBrush(Color.FromArgb(18, 45, 75))) g.FillRectangle(background, search);
        g.DrawRectangle(Pens.Blue, search);
        using (var red = new SolidBrush(Color.FromArgb(220, 30, 30))) g.FillRectangle(red, 75, 90, 45, 45);
        g.DrawString("Fixed red pixel", Font, Brushes.Black, 55, 150);
        if (present) g.DrawImageUnscaled(icon, search.X + 12 + position, search.Y + 68);
    }

    [STAThread]
    static void Main() {
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        Application.Run(new Fixture());
    }
}
'@ | Set-Content -LiteralPath $sourcePath -Encoding UTF8

@'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" name="IconSearchFixture"/>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
    </windowsSettings>
  </application>
</assembly>
'@ | Set-Content -LiteralPath $manifestPath -Encoding UTF8

$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
& $compiler /nologo /target:winexe "/out:$exePath" "/win32manifest:$manifestPath" /reference:System.Drawing.dll /reference:System.Windows.Forms.dll $sourcePath
if ($LASTEXITCODE -ne 0) { throw "Fixture compilation failed ($LASTEXITCODE)" }
Write-Output $exePath
if ($Launch) { Start-Process -FilePath $exePath }
