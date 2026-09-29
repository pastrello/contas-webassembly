using Caixa.Core;
using Caixa.Core.Models;
using Caixa.Core.Utilities;
using Caixa.WinForms.Utilities;

namespace Caixa.WinForms.Forms;

/// <summary>Relatório por históricos com exportação CSV e PDF.</summary>
public sealed class HistoryReportForm : Form
{
    private readonly CaixaApplication _app;
    private readonly ComboBox _account = new();
    private readonly DateTimePicker _start = new();
    private readonly DateTimePicker _end = new();
    private readonly CheckedListBox _histories = new();
    private readonly DataGridView _grid = new();
    private readonly Label _summary = new();
    private PdfReportExporter.Options? _reportOptions;

    public HistoryReportForm(CaixaApplication app)
    {
        _app = app;
        Text = "Relatório de lançamentos por histórico";
        StartPosition = FormStartPosition.CenterParent;
        ClientSize = new Size(1160, 650);
        MinimumSize = new Size(1000, 580);
        BuildUi();
        Load += (_, _) => LoadData();
    }

    private void BuildUi()
    {
        Controls.Add(new Label { Text = "Conta", Location = new Point(18, 15), AutoSize = true });
        _account.Location = new Point(18, 37);
        _account.Size = new Size(350, 23);
        _account.DropDownStyle = ComboBoxStyle.DropDownList;

        Controls.Add(new Label { Text = "De", Location = new Point(390, 15), AutoSize = true });
        _start.Location = new Point(390, 37);
        _start.Size = new Size(105, 23);
        _start.Format = DateTimePickerFormat.Short;

        Controls.Add(new Label { Text = "Até", Location = new Point(510, 15), AutoSize = true });
        _end.Location = new Point(510, 37);
        _end.Size = new Size(105, 23);
        _end.Format = DateTimePickerFormat.Short;

        var query = new Button { Text = "Consultar", Location = new Point(635, 36), Size = new Size(90, 25) };
        query.Click += (_, _) => Query();

        var exportCsv = new Button { Text = "Exportar CSV", Location = new Point(735, 36), Size = new Size(100, 25) };
        exportCsv.Click += (_, _) => ExportCsv();

        var exportPdf = new Button { Text = "Gerar PDF", Location = new Point(845, 36), Size = new Size(100, 25) };
        exportPdf.Click += (_, _) => ExportPdf();

        Controls.Add(new Label { Text = "Históricos", Location = new Point(18, 80), AutoSize = true });
        _histories.Location = new Point(18, 105);
        _histories.Size = new Size(260, 450);
        _histories.CheckOnClick = true;
        _histories.Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left;

        var selectAll = new Button { Text = "Marcar todos", Location = new Point(18, 565), Size = new Size(110, 25), Anchor = AnchorStyles.Bottom | AnchorStyles.Left };
        selectAll.Click += (_, _) => SetAllHistories(true);
        var clearAll = new Button { Text = "Limpar", Location = new Point(138, 565), Size = new Size(85, 25), Anchor = AnchorStyles.Bottom | AnchorStyles.Left };
        clearAll.Click += (_, _) => SetAllHistories(false);

        UiHelpers.ConfigureGrid(_grid);
        _grid.Location = new Point(295, 80);
        _grid.Size = new Size(840, 475);
        _grid.Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;
        _grid.Columns.Add(UiHelpers.TextColumn("Data", nameof(Row.Date), 90));
        _grid.Columns.Add(UiHelpers.TextColumn("Doc.", nameof(Row.Document), 80));
        _grid.Columns.Add(UiHelpers.TextColumn("Histórico", nameof(Row.History), 180));
        _grid.Columns.Add(UiHelpers.TextColumn("Complemento", nameof(Row.Description), 320));
        _grid.Columns.Add(UiHelpers.MoneyColumn("Entrada", nameof(Row.Credit), 110));
        _grid.Columns.Add(UiHelpers.MoneyColumn("Saída", nameof(Row.Debit), 110));

        _summary.Location = new Point(295, 575);
        _summary.Size = new Size(720, 25);
        _summary.Anchor = AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;

        var close = new Button { Text = "Fechar", Location = new Point(1050, 570), Size = new Size(85, 27), Anchor = AnchorStyles.Bottom | AnchorStyles.Right };
        close.Click += (_, _) => Close();

        Controls.AddRange([_account, _start, _end, query, exportCsv, exportPdf, _histories, selectAll, clearAll, _grid, _summary, close]);
    }

    private void LoadData()
    {
        _account.DataSource = _app.Accounts.GetAll(true);
        _start.Value = DateTime.Today.AddMonths(-1);
        _end.Value = DateTime.Today;
        _histories.Items.Clear();
        foreach (var h in _app.Histories.GetAll(true))
            _histories.Items.Add(h, false);
    }

    private void SetAllHistories(bool value)
    {
        for (var i = 0; i < _histories.Items.Count; i++)
            _histories.SetItemChecked(i, value);
    }

    private void Query()
    {
        if (_account.SelectedItem is not Account account) return;
        var start = DateOnly.FromDateTime(_start.Value.Date);
        var end = DateOnly.FromDateTime(_end.Value.Date);
        if (end < start)
        {
            MessageBox.Show(this, "A data final deve ser igual ou posterior à inicial.");
            return;
        }

        var ids = _histories.CheckedItems.Cast<HistoryItem>().Select(h => h.Id).ToArray();
        if (ids.Length == 0)
        {
            MessageBox.Show(this, "Marque ao menos um histórico.");
            return;
        }

        var data = _app.Reports.ByHistories(account.Id, start, end, ids)
            .Where(t => !t.LegacyRolledIntoOpeningBalance)
            .Select(t => new Row
            {
                Date = t.Date.ToString("dd/MM/yyyy"),
                Document = t.DocumentNumber,
                History = $"{t.HistoryCode} - {t.HistoryDescription}",
                Description = t.Description,
                Credit = t.Direction > 0 ? Money.FromCents(t.AmountCents) : 0,
                Debit = t.Direction < 0 ? Money.FromCents(t.AmountCents) : 0
            }).ToList();

        _grid.DataSource = data;
        _summary.Text = $"{data.Count} registro(s) | Entradas: {data.Sum(x => x.Credit):C2} | Saídas: {data.Sum(x => x.Debit):C2} | Diferença: {data.Sum(x => x.Credit) - data.Sum(x => x.Debit):C2}";
        var historyText = string.Join(", ", _histories.CheckedItems.Cast<HistoryItem>().Select(h => $"{h.Code} - {h.Description}"));
        _reportOptions = new PdfReportExporter.Options(_app.Settings.Get().CompanyName,
            Text, account.ToString(), start, end, $"Históricos: {historyText}", _summary.Text);
    }

    private void ExportCsv()
    {
        if (_grid.Rows.Count == 0) return;
        using var dlg = new SaveFileDialog
        {
            Filter = "CSV (*.csv)|*.csv",
            FileName = $"historicos-{DateTime.Now:yyyyMMdd}.csv"
        };
        if (dlg.ShowDialog(this) != DialogResult.OK) return;
        try
        {
            CsvExporter.Export(_grid, dlg.FileName);
            MessageBox.Show(this, "Arquivo exportado.");
        }
        catch (Exception ex)
        {
            UiHelpers.ShowError(this, ex);
        }
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
            Title = "Salvar relatório em PDF",
            Filter = "PDF (*.pdf)|*.pdf",
            DefaultExt = "pdf",
            AddExtension = true,
            FileName = $"historicos-{options.StartDate:yyyyMMdd}-{options.EndDate:yyyyMMdd}.pdf"
        };
        if (dlg.ShowDialog(this) != DialogResult.OK) return;

        try
        {
            Cursor = Cursors.WaitCursor;
            PdfReportExporter.Export(_grid, dlg.FileName, options);
        }
        catch (Exception ex)
        {
            UiHelpers.ShowError(this, ex);
            return;
        }
        finally
        {
            Cursor = Cursors.Default;
        }

        PdfReportExporter.OfferOpen(this, dlg.FileName);
    }

    private sealed class Row
    {
        public string Date { get; init; } = "";
        public string Document { get; init; } = "";
        public string History { get; init; } = "";
        public string Description { get; init; } = "";
        public decimal Credit { get; init; }
        public decimal Debit { get; init; }
    }
}
