using Caixa.Core;
using Caixa.Core.Models;
using Caixa.Core.Utilities;
using Caixa.WinForms.Utilities;

namespace Caixa.WinForms.Forms;

public enum PeriodReportMode { Statement, Consistency, CheckForecast, Archived }

/// <summary>Tela reutilizável para relatórios de período, com visualização, CSV e PDF.</summary>
public sealed class PeriodReportForm : Form
{
    private readonly CaixaApplication _app;
    private readonly PeriodReportMode _mode;
    private readonly ComboBox _account = new();
    private readonly DateTimePicker _start = new();
    private readonly DateTimePicker _end = new();
    private readonly DataGridView _grid = new();
    private readonly Label _summary = new();
    private readonly Button _restore = new();
    private PdfReportExporter.Options? _reportOptions;

    public PeriodReportForm(CaixaApplication app, PeriodReportMode mode)
    {
        _app = app;
        _mode = mode;
        Text = mode switch
        {
            PeriodReportMode.Statement => "Extrato",
            PeriodReportMode.Consistency => "Consistência / lançamentos não contabilizados",
            PeriodReportMode.CheckForecast => "Previsão de cheques",
            PeriodReportMode.Archived => "Lançamentos baixados / arquivados",
            _ => "Relatório"
        };
        StartPosition = FormStartPosition.CenterParent;
        ClientSize = new Size(1100, 620);
        MinimumSize = new Size(980, 560);
        BuildUi();
        Load += (_, _) =>
        {
            _account.DataSource = _app.Accounts.GetAll(true);
            _start.Value = DateTime.Today.AddMonths(-1);
            _end.Value = DateTime.Today;
            LoadReport();
        };
    }

    private void BuildUi()
    {
        Controls.Add(new Label { Text = "Conta", Location = new Point(18, 15), AutoSize = true });
        _account.Location = new Point(18, 37); _account.Size = new Size(365, 23); _account.DropDownStyle = ComboBoxStyle.DropDownList;
        Controls.Add(new Label { Text = "De", Location = new Point(400, 15), AutoSize = true });
        _start.Location = new Point(400, 37); _start.Size = new Size(105, 23); _start.Format = DateTimePickerFormat.Short;
        Controls.Add(new Label { Text = "Até", Location = new Point(520, 15), AutoSize = true });
        _end.Location = new Point(520, 37); _end.Size = new Size(105, 23); _end.Format = DateTimePickerFormat.Short;
        var load = new Button { Text = "Atualizar", Location = new Point(645, 36), Size = new Size(85, 25) };
        load.Click += (_, _) => LoadReport();
        var exportCsv = new Button { Text = "Exportar CSV", Location = new Point(740, 36), Size = new Size(100, 25) };
        exportCsv.Click += (_, _) => ExportCsv();
        var exportPdf = new Button { Text = "Gerar PDF", Location = new Point(850, 36), Size = new Size(100, 25) };
        exportPdf.Click += (_, _) => ExportPdf();

        UiHelpers.ConfigureGrid(_grid);
        _grid.Location = new Point(18, 80); _grid.Size = new Size(1057, 470);
        _grid.Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;
        ConfigureColumns();

        _summary.Location = new Point(18, 570); _summary.Size = new Size(790, 25);
        _summary.Anchor = AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;
        _restore.Text = "Restaurar selecionado"; _restore.Location = new Point(820, 565); _restore.Size = new Size(155, 27);
        _restore.Anchor = AnchorStyles.Bottom | AnchorStyles.Right; _restore.Visible = _mode == PeriodReportMode.Archived;
        _restore.Click += (_, _) => Restore();
        var close = new Button { Text = "Fechar", Location = new Point(990, 565), Size = new Size(85, 27), Anchor = AnchorStyles.Bottom | AnchorStyles.Right };
        close.Click += (_, _) => Close();
        Controls.AddRange([_account, _start, _end, load, exportCsv, exportPdf, _restore, _grid, _summary, close]);
    }

    private void ConfigureColumns()
    {
        _grid.Columns.Clear();
        _grid.Columns.Add(UiHelpers.TextColumn("Data", nameof(ReportRow.Date), 90));
        _grid.Columns.Add(UiHelpers.TextColumn("Doc.", nameof(ReportRow.Document), 80));
        _grid.Columns.Add(UiHelpers.TextColumn("Histórico", nameof(ReportRow.History), 180));
        _grid.Columns.Add(UiHelpers.TextColumn("Complemento", nameof(ReportRow.Description), _mode == PeriodReportMode.Statement ? 300 : 360));
        _grid.Columns.Add(UiHelpers.MoneyColumn("Entrada", nameof(ReportRow.Credit), 115));
        _grid.Columns.Add(UiHelpers.MoneyColumn("Saída", nameof(ReportRow.Debit), 115));
        if (_mode == PeriodReportMode.Statement)
            _grid.Columns.Add(UiHelpers.MoneyColumn("Saldo", nameof(ReportRow.Balance), 125));
        _grid.Columns.Add(UiHelpers.CheckColumn("Contab.", nameof(ReportRow.Cleared), 65));
    }

    private Account? Account => _account.SelectedItem as Account;

    private void LoadReport()
    {
        if (Account is null) return;
        var start = DateOnly.FromDateTime(_start.Value.Date);
        var end = DateOnly.FromDateTime(_end.Value.Date);
        if (end < start)
        {
            MessageBox.Show(this, "A data final deve ser igual ou posterior à inicial.");
            return;
        }

        List<ReportRow> rows;
        if (_mode == PeriodReportMode.Statement)
        {
            rows = _app.Reports.Statement(Account.Id, start, end).Select(x => new ReportRow
            {
                Id = x.TransactionId, Date = x.Date.ToString("dd/MM/yyyy"), Document = x.DocumentNumber,
                History = $"{x.HistoryCode} - {x.HistoryDescription}", Description = x.Description,
                Credit = x.Direction > 0 ? Money.FromCents(x.AmountCents) : 0m,
                Debit = x.Direction < 0 ? Money.FromCents(x.AmountCents) : 0m,
                Balance = Money.FromCents(x.RunningBalanceCents), Cleared = x.IsCleared,
                Archived = x.IsArchived
            }).ToList();
        }
        else
        {
            var data = _mode switch
            {
                PeriodReportMode.Consistency => _app.Reports.Consistency(Account.Id, start, end),
                PeriodReportMode.CheckForecast => _app.Reports.CheckForecast(Account.Id, start, end),
                PeriodReportMode.Archived => _app.Reports.Archived(Account.Id, start, end),
                _ => []
            };
            rows = data.Select(x => new ReportRow
            {
                Id = x.Id, Date = x.Date.ToString("dd/MM/yyyy"), Document = x.DocumentNumber,
                History = $"{x.HistoryCode} - {x.HistoryDescription}", Description = x.Description,
                Credit = x.Direction > 0 ? Money.FromCents(x.AmountCents) : 0m,
                Debit = x.Direction < 0 ? Money.FromCents(x.AmountCents) : 0m,
                Cleared = x.IsCleared, Archived = x.IsArchived,
                ProtectedCompatibilityRecord = x.LegacyRolledIntoOpeningBalance
            }).ToList();
        }

        _grid.DataSource = rows;
        var credit = rows.Sum(r => r.Credit); var debit = rows.Sum(r => r.Debit);
        if (_mode == PeriodReportMode.Statement)
        {
            var finalBalance = rows.Count > 0 ? rows[^1].Balance : Money.FromCents(_app.Balances.CalculateBefore(Account.Id, start));
            _summary.Text = $"{rows.Count} registro(s) | Entradas: {credit:C2} | Saídas: {debit:C2} | Movimento: {credit - debit:C2} | Saldo final: {finalBalance:C2}";
        }
        else
            _summary.Text = $"{rows.Count} registro(s) | Entradas: {credit:C2} | Saídas: {debit:C2} | Diferença: {credit - debit:C2}";
        // Cabeçalho e linhas pertencem à mesma consulta, mesmo se os filtros mudarem depois.
        _reportOptions = new PdfReportExporter.Options(_app.Settings.Get().CompanyName,
            Text, Account.ToString(), start, end, ReportDescription(), _summary.Text);
    }

    private void ExportCsv()
    {
        if (_grid.Rows.Count == 0) return;
        using var dlg = new SaveFileDialog { Filter = "CSV (*.csv)|*.csv", FileName = $"{SafeFileName(Text)}-{DateTime.Now:yyyyMMdd}.csv" };
        if (dlg.ShowDialog(this) != DialogResult.OK) return;
        try { CsvExporter.Export(_grid, dlg.FileName); MessageBox.Show(this, "Arquivo exportado."); }
        catch (Exception ex) { UiHelpers.ShowError(this, ex); }
    }

    private void ExportPdf()
    {
        if (_reportOptions is not { } options || _grid.Rows.Count == 0)
        {
            MessageBox.Show(this, "Não há dados para gerar o PDF.", "Relatório PDF", MessageBoxButtons.OK, MessageBoxIcon.Information);
            return;
        }
        using var dlg = new SaveFileDialog
        {
            Title = "Salvar relatório em PDF", Filter = "PDF (*.pdf)|*.pdf", DefaultExt = "pdf", AddExtension = true,
            FileName = $"{SafeFileName(Text)}-{options.StartDate:yyyyMMdd}-{options.EndDate:yyyyMMdd}.pdf"
        };
        if (dlg.ShowDialog(this) != DialogResult.OK) return;
        try
        {
            Cursor = Cursors.WaitCursor;
            PdfReportExporter.Export(_grid, dlg.FileName, options);
        }
        catch (Exception ex) { UiHelpers.ShowError(this, ex); return; }
        finally { Cursor = Cursors.Default; }
        PdfReportExporter.OfferOpen(this, dlg.FileName);
    }

    private string ReportDescription() => _mode switch
    {
        PeriodReportMode.Statement => "Extrato com saldo acumulado no período.",
        PeriodReportMode.Consistency => "Somente lançamentos ativos ainda não contabilizados.",
        PeriodReportMode.CheckForecast => "Previsão: histórico 51 ainda não contabilizado.",
        PeriodReportMode.Archived => "Lançamentos baixados / arquivados no período.",
        _ => string.Empty
    };

    private static string SafeFileName(string value)
    {
        foreach (var c in Path.GetInvalidFileNameChars()) value = value.Replace(c, '-');
        return value.ToLowerInvariant().Replace(' ', '-').Replace('/', '-');
    }

    private void Restore()
    {
        if (_mode != PeriodReportMode.Archived || _grid.CurrentRow?.DataBoundItem is not ReportRow row) return;
        if (row.ProtectedCompatibilityRecord)
        {
            MessageBox.Show(this, "Este registro é mantido apenas para compatibilidade com dados existentes e permanece arquivado.");
            return;
        }
        if (MessageBox.Show(this, "Restaurar este lançamento para a movimentação ativa?", "Confirmação", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
        _app.Transactions.Unarchive(row.Id);
        LoadReport();
    }

    private sealed class ReportRow
    {
        public long Id { get; init; }
        public string Date { get; init; } = "";
        public string Document { get; init; } = "";
        public string History { get; init; } = "";
        public string Description { get; init; } = "";
        public decimal Credit { get; init; }
        public decimal Debit { get; init; }
        public decimal Balance { get; init; }
        public bool Cleared { get; init; }
        public bool Archived { get; init; }
        public bool ProtectedCompatibilityRecord { get; init; }
    }
}
