using Caixa.Core;
using Caixa.Core.Models;
using Caixa.Core.Utilities;
using Caixa.WinForms.Utilities;

namespace Caixa.WinForms.Forms;

/// <summary>Janela principal com menus e painel de saldos das contas.</summary>
public partial class MainForm : Form
{
    private readonly CaixaApplication _app;

    public MainForm(CaixaApplication app)
    {
        _app = app;
        InitializeComponent();
        UiHelpers.ConfigureGrid(gridAccounts);
        gridAccounts.Columns.Add(UiHelpers.TextColumn("Código", nameof(AccountDashboardRow.Code), 65));
        gridAccounts.Columns.Add(UiHelpers.TextColumn("Conta", nameof(AccountDashboardRow.Number), 110));
        gridAccounts.Columns.Add(UiHelpers.TextColumn("Descrição / Banco", nameof(AccountDashboardRow.Name), 250));
        gridAccounts.Columns.Add(UiHelpers.MoneyColumn("Saldo inicial", nameof(AccountDashboardRow.Opening), 130));
        gridAccounts.Columns.Add(UiHelpers.MoneyColumn("Saldo bancário", nameof(AccountDashboardRow.Cleared), 140));
        gridAccounts.Columns.Add(UiHelpers.MoneyColumn("Saldo real", nameof(AccountDashboardRow.Real), 140));
        gridAccounts.Columns.Add(UiHelpers.MoneyColumn("Pendente", nameof(AccountDashboardRow.Pending), 120));

        Shown += (_, _) => RefreshDashboard();
    }

    private void RefreshDashboard()
    {
        var settings = _app.Settings.Get();
        Text = $"Caixa Modernizado - {settings.CompanyName}";
        lblCompany.Text = settings.CompanyName;
        lblDatabase.Text = $"Banco: {_app.Database.DatabasePath}";

        var rows = new List<AccountDashboardRow>();
        foreach (var account in _app.Accounts.GetAll())
        {
            var b = _app.Balances.Calculate(account.Id);
            rows.Add(new AccountDashboardRow
            {
                Code = account.Code,
                Number = account.Number,
                Name = account.Name,
                Opening = Money.FromCents(b.OpeningBalanceCents),
                Cleared = Money.FromCents(b.ClearedBalanceCents),
                Real = Money.FromCents(b.RealBalanceCents),
                Pending = Money.FromCents(b.PendingNetCents)
            });
        }
        gridAccounts.DataSource = rows;
        lblAccountCount.Text = $"{rows.Count} conta(s)";
    }

    private void OpenDialog(Form form)
    {
        using (form) form.ShowDialog(this);
    }

    private void OpenAndRefresh(Form form)
    {
        using (form)
            form.ShowDialog(this);
        RefreshDashboard();
    }

    private void mnuAccounts_Click(object? sender, EventArgs e) => OpenAndRefresh(new AccountsForm(_app));
    private void mnuHistories_Click(object? sender, EventArgs e) => OpenAndRefresh(new HistoriesForm(_app));
    private void mnuSettings_Click(object? sender, EventArgs e) => OpenAndRefresh(new SettingsForm(_app));
    private void mnuTransactions_Click(object? sender, EventArgs e) => OpenAndRefresh(new TransactionsForm(_app));
    private void mnuTransactionSearch_Click(object? sender, EventArgs e) => OpenDialog(new TransactionSearchForm(_app));
    private void mnuStatement_Click(object? sender, EventArgs e) => OpenDialog(new PeriodReportForm(_app, PeriodReportMode.Statement));
    private void mnuConsistency_Click(object? sender, EventArgs e) => OpenDialog(new PeriodReportForm(_app, PeriodReportMode.Consistency));
    private void mnuCheckForecast_Click(object? sender, EventArgs e) => OpenDialog(new PeriodReportForm(_app, PeriodReportMode.CheckForecast));
    private void mnuArchive_Click(object? sender, EventArgs e) => OpenAndRefresh(new ArchiveForm(_app));
    private void mnuArchived_Click(object? sender, EventArgs e) => OpenDialog(new PeriodReportForm(_app, PeriodReportMode.Archived));
    private void mnuHistoryReport_Click(object? sender, EventArgs e) => OpenDialog(new HistoryReportForm(_app));
    private void mnuMaintenance_Click(object? sender, EventArgs e) => OpenDialog(new DatabaseMaintenanceForm(_app));

    private void mnuBackup_Click(object? sender, EventArgs e)
    {
        using var dlg = new FolderBrowserDialog
        {
            Description = "Escolha a pasta onde o backup compactado será salvo.",
            ShowNewFolderButton = true,
            UseDescriptionForTitle = true
        };

        if (dlg.ShowDialog(this) != DialogResult.OK) return;

        try
        {
            Cursor = Cursors.WaitCursor;
            var result = _app.Backup.CreateCompressed(dlg.SelectedPath);

            MessageBox.Show(this,
                $"Backup criado e verificado com sucesso.\r\n\r\n" +
                $"Arquivo: {result.ZipPath}\r\n" +
                $"SQLite: {FormatBytes(result.DatabaseSizeBytes)}\r\n" +
                $"ZIP: {FormatBytes(result.ZipSizeBytes)}\r\n" +
                $"Contas: {result.AccountCount}\r\n" +
                $"Históricos: {result.HistoryCount}\r\n" +
                $"Lançamentos: {result.TransactionCount}\r\n" +
                $"Integridade: {result.QuickCheckResult}",
                "Backup", MessageBoxButtons.OK, MessageBoxIcon.Information);
        }
        catch (Exception ex)
        {
            UiHelpers.ShowError(this, ex);
        }
        finally
        {
            Cursor = Cursors.Default;
        }
    }

    private void mnuRestore_Click(object? sender, EventArgs e)
    {
        using var dlg = new OpenFileDialog
        {
            Title = "Selecione o backup do Caixa Modernizado",
            Filter = "Backup do Caixa Modernizado (*.zip)|*.zip|Todos os arquivos (*.*)|*.*",
            CheckFileExists = true,
            Multiselect = false
        };

        if (dlg.ShowDialog(this) != DialogResult.OK) return;

        var answer = MessageBox.Show(this,
            "A restauração substituirá os dados atuais pelos dados existentes no backup selecionado.\r\n\r\n" +
            "Antes da restauração o programa criará automaticamente um backup de segurança do banco atual.\r\n\r\n" +
            $"Backup selecionado:\r\n{dlg.FileName}\r\n\r\nDeseja continuar?",
            "Confirmar restauração",
            MessageBoxButtons.YesNo,
            MessageBoxIcon.Warning,
            MessageBoxDefaultButton.Button2);

        if (answer != DialogResult.Yes) return;

        try
        {
            Cursor = Cursors.WaitCursor;
            var result = _app.Backup.RestoreCompressed(dlg.FileName);
            RefreshDashboard();

            MessageBox.Show(this,
                "Backup restaurado e validado com sucesso.\r\n\r\n" +
                $"Origem: {result.SourceZipPath}\r\n\r\n" +
                $"Backup preventivo do banco anterior:\r\n{result.SafetyBackupPath}\r\n\r\n" +
                $"Contas: {result.AccountCount}\r\n" +
                $"Históricos: {result.HistoryCount}\r\n" +
                $"Lançamentos: {result.TransactionCount}\r\n" +
                $"Integridade: {result.QuickCheckResult}\r\n" +
                $"Esquema: user_version={result.UserVersion}",
                "Restauração concluída",
                MessageBoxButtons.OK,
                MessageBoxIcon.Information);
        }
        catch (Exception ex)
        {
            UiHelpers.ShowError(this, ex);
        }
        finally
        {
            Cursor = Cursors.Default;
        }
    }

    private static string FormatBytes(long bytes)
    {
        string[] units = ["B", "KB", "MB", "GB", "TB"];
        double value = bytes;
        var unit = 0;
        while (value >= 1024 && unit < units.Length - 1)
        {
            value /= 1024;
            unit++;
        }
        return $"{value:N2} {units[unit]}";
    }

    private void mnuAbout_Click(object? sender, EventArgs e) => OpenDialog(new AboutForm());
    private void mnuExit_Click(object? sender, EventArgs e) => Close();

    private sealed class AccountDashboardRow
    {
        public string Code { get; init; } = string.Empty;
        public string Number { get; init; } = string.Empty;
        public string Name { get; init; } = string.Empty;
        public decimal Opening { get; init; }
        public decimal Cleared { get; init; }
        public decimal Real { get; init; }
        public decimal Pending { get; init; }
    }
}
