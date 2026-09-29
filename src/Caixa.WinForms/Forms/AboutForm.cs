namespace Caixa.WinForms.Forms;

/// <summary>Informações sobre a aplicação.</summary>
public sealed class AboutForm : Form
{
    public AboutForm()
    {
        Text = "Sobre";
        StartPosition = FormStartPosition.CenterParent;
        ClientSize = new Size(560, 260);

        var text = new Label
        {
            Location = new Point(25, 25),
            Size = new Size(510, 175),
            Text =
                $"CAIXA MODERNIZADO — versão {Application.ProductVersion}\r\n\r\n" +
                "Controle simples de contas financeiras.\r\n\r\n" +
                "C# / .NET 10 / WinForms / SQLite\r\n" +
                "Relatórios PDF, backup/restauração e manutenção do banco."
        };

        var close = new Button { Text = "Fechar", Location = new Point(455, 210) };
        close.Click += (_, _) => Close();
        Controls.AddRange([text, close]);
    }
}
