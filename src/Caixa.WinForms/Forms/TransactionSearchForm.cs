using Caixa.Core;
using Caixa.Core.Models;
using Caixa.Core.Utilities;
using Caixa.WinForms.Utilities;

namespace Caixa.WinForms.Forms;

/// <summary>Consulta flexível e somente leitura de lançamentos, com exportação CSV e PDF.</summary>
public sealed class TransactionSearchForm : Form
{
    private readonly CaixaApplication _app;
    private readonly ComboBox _account = new();
    private readonly ComboBox _history = new();
    private readonly CheckBox _usePeriod = new();
    private readonly DateTimePicker _start = new();
    private readonly DateTimePicker _end = new();
    private readonly TextBox _document = new();
    private readonly TextBox _description = new();
    private readonly ComboBox _cleared = new();
    private readonly ComboBox _archived = new();
    private readonly NumericUpDown _minAmount = new();
    private readonly NumericUpDown _maxAmount = new();
    private readonly DataGridView _grid = new();
    private readonly Label _summary = new();
    private PdfReportExporter.Options? _reportOptions;

    public TransactionSearchForm(CaixaApplication app)
    {
        _app = app;
        Text = "Consulta avançada de lançamentos";
        StartPosition = FormStartPosition.CenterParent;
        ClientSize = new Size(1280, 720);
        MinimumSize = new Size(1080, 650);
        BuildUi();
        Load += (_, _) => { LoadFilters(); Search(); };
    }

    private void BuildUi()
    {
        var filters = new GroupBox { Text = "Filtros", Location = new Point(15, 12), Size = new Size(1250, 175), Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right };
        filters.Controls.Add(new Label { Text = "Conta", Location = new Point(15, 27), AutoSize = true });
        _account.Location = new Point(15, 49); _account.Size = new Size(305, 23); _account.DropDownStyle = ComboBoxStyle.DropDownList;
        filters.Controls.Add(new Label { Text = "Histórico", Location = new Point(335, 27), AutoSize = true });
        _history.Location = new Point(335, 49); _history.Size = new Size(300, 23); _history.DropDownStyle = ComboBoxStyle.DropDownList;
        _usePeriod.Text = "Filtrar período"; _usePeriod.Location = new Point(650, 27); _usePeriod.AutoSize = true; _usePeriod.Checked = true;
        _start.Location = new Point(650, 49); _start.Size = new Size(105, 23); _start.Format = DateTimePickerFormat.Short;
        filters.Controls.Add(new Label { Text = "até", Location = new Point(762, 53), AutoSize = true });
        _end.Location = new Point(790, 49); _end.Size = new Size(105, 23); _end.Format = DateTimePickerFormat.Short;
        _usePeriod.CheckedChanged += (_, _) => { _start.Enabled = _usePeriod.Checked; _end.Enabled = _usePeriod.Checked; };
        filters.Controls.Add(new Label { Text = "Situação", Location = new Point(910, 27), AutoSize = true });
        _cleared.Location = new Point(910, 49); _cleared.Size = new Size(145, 23); _cleared.DropDownStyle = ComboBoxStyle.DropDownList;
        filters.Controls.Add(new Label { Text = "Baixa", Location = new Point(1070, 27), AutoSize = true });
        _archived.Location = new Point(1070, 49); _archived.Size = new Size(155, 23); _archived.DropDownStyle = ComboBoxStyle.DropDownList;
        filters.Controls.Add(new Label { Text = "Documento contém", Location = new Point(15, 84), AutoSize = true });
        _document.Location = new Point(15, 106); _document.Size = new Size(190, 23); _document.MaxLength = 60;
        filters.Controls.Add(new Label { Text = "Complemento contém", Location = new Point(220, 84), AutoSize = true });
        _description.Location = new Point(220, 106); _description.Size = new Size(415, 23); _description.MaxLength = 250;
        filters.Controls.Add(new Label { Text = "Valor mínimo", Location = new Point(650, 84), AutoSize = true });
        ConfigureMoney(_minAmount, new Point(650, 106));
        filters.Controls.Add(new Label { Text = "Valor máximo", Location = new Point(790, 84), AutoSize = true });
        ConfigureMoney(_maxAmount, new Point(790, 106));

        var search = new Button { Text = "Pesquisar", Location = new Point(15, 140), Size = new Size(95, 27) };
        search.Click += (_, _) => Search();
        var clear = new Button { Text = "Limpar filtros", Location = new Point(120, 140), Size = new Size(105, 27) };
        clear.Click += (_, _) => ResetFilters();
        var csv = new Button { Text = "Exportar CSV", Location = new Point(235, 140), Size = new Size(105, 27) };
        csv.Click += (_, _) => ExportCsv();
        var pdf = new Button { Text = "Gerar PDF", Location = new Point(350, 140), Size = new Size(95, 27) };
        pdf.Click += (_, _) => ExportPdf();
        filters.Controls.AddRange([_account, _history, _usePeriod, _start, _end, _cleared, _archived, _document, _description, _minAmount, _maxAmount, search, clear, csv, pdf]);

        UiHelpers.ConfigureGrid(_grid);
        _grid.Location = new Point(15, 198); _grid.Size = new Size(1250, 465);
        _grid.Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;
        _grid.Columns.Add(UiHelpers.TextColumn("Conta", nameof(SearchRow.Account), 135));
        _grid.Columns.Add(UiHelpers.TextColumn("Data", nameof(SearchRow.Date), 90));
        _grid.Columns.Add(UiHelpers.TextColumn("Doc.", nameof(SearchRow.Document), 90));
        _grid.Columns.Add(UiHelpers.TextColumn("Histórico", nameof(SearchRow.History), 185));
        _grid.Columns.Add(UiHelpers.TextColumn("Complemento", nameof(SearchRow.Description), 330));
        _grid.Columns.Add(UiHelpers.MoneyColumn("Entrada", nameof(SearchRow.Credit), 115));
        _grid.Columns.Add(UiHelpers.MoneyColumn("Saída", nameof(SearchRow.Debit), 115));
        _grid.Columns.Add(UiHelpers.CheckColumn("Contab.", nameof(SearchRow.Cleared), 65));
        _grid.Columns.Add(UiHelpers.CheckColumn("Baixado", nameof(SearchRow.Archived), 65));

        _summary.Location = new Point(15, 675); _summary.Size = new Size(1100, 25); _summary.Anchor = AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;
        var close = new Button { Text = "Fechar", Location = new Point(1180, 671), Size = new Size(85, 27), Anchor = AnchorStyles.Bottom | AnchorStyles.Right };
        close.Click += (_, _) => Close();
        Controls.AddRange([filters, _grid, _summary, close]);
        AcceptButton = search;
    }

    private static void ConfigureMoney(NumericUpDown control, Point location)
    {
        control.Location = location; control.Size = new Size(125, 23); control.DecimalPlaces = 2;
        control.ThousandsSeparator = true; control.Minimum = 0; control.Maximum = 999999999999m;
    }

    private void LoadFilters()
    {
        _account.Items.Clear(); _account.Items.Add(new AccountFilter(null, "Todas as contas"));
        foreach (var account in _app.Accounts.GetAll()) _account.Items.Add(new AccountFilter(account.Id, $"{account.Code} - {account.Number} - {account.Name}"));
        _account.SelectedIndex = 0;
        _history.Items.Clear(); _history.Items.Add(new HistoryFilter(null, "Todos os históricos"));
        foreach (var history in _app.Histories.GetAll()) _history.Items.Add(new HistoryFilter(history.Id, $"{history.Code} - {history.Description}"));
        _history.SelectedIndex = 0;
        _cleared.Items.AddRange([new BoolFilter(null, "Todos"), new BoolFilter(true, "Contabilizados"), new BoolFilter(false, "Não contabilizados")]); _cleared.SelectedIndex = 0;
        _archived.Items.AddRange([new BoolFilter(false, "Somente ativos"), new BoolFilter(true, "Somente baixados"), new BoolFilter(null, "Ativos e baixados")]); _archived.SelectedIndex = 0;
        _start.Value = DateTime.Today.AddMonths(-1); _end.Value = DateTime.Today;
    }

    private void ResetFilters()
    {
        _account.SelectedIndex = 0; _history.SelectedIndex = 0; _usePeriod.Checked = true;
        _start.Value = DateTime.Today.AddMonths(-1); _end.Value = DateTime.Today;
        _document.Clear(); _description.Clear(); _cleared.SelectedIndex = 0; _archived.SelectedIndex = 0;
        _minAmount.Value = 0; _maxAmount.Value = 0; Search();
    }

    private TransactionSearchCriteria BuildCriteria()
    {
        var account = (AccountFilter)_account.SelectedItem!;
        var history = (HistoryFilter)_history.SelectedItem!;
        var cleared = (BoolFilter)_cleared.SelectedItem!;
        var archived = (BoolFilter)_archived.SelectedItem!;
        return new TransactionSearchCriteria
        {
            AccountId = account.Id,
            HistoryId = history.Id,
            StartDate = _usePeriod.Checked ? DateOnly.FromDateTime(_start.Value.Date) : null,
            EndDate = _usePeriod.Checked ? DateOnly.FromDateTime(_end.Value.Date) : null,
            DocumentContains = _document.Text,
            DescriptionContains = _description.Text,
            Cleared = cleared.Value,
            Archived = archived.Value,
            MinimumAmountCents = _minAmount.Value > 0 ? Money.ToCents(_minAmount.Value) : null,
            MaximumAmountCents = _maxAmount.Value > 0 ? Money.ToCents(_maxAmount.Value) : null
        };
    }

    private void Search()
    {
        if (_account.SelectedItem is null) return;
        try
        {
            Cursor = Cursors.WaitCursor;
            var criteria = BuildCriteria();
            var rows = _app.Reports.SearchTransactions(criteria).Select(t => new SearchRow
            {
                Account = $"{t.AccountCode} - {t.AccountName}", Date = t.Date.ToString("dd/MM/yyyy"), Document = t.DocumentNumber,
                History = $"{t.HistoryCode} - {t.HistoryDescription}", Description = t.Description,
                Credit = t.Direction > 0 ? Money.FromCents(t.AmountCents) : 0m, Debit = t.Direction < 0 ? Money.FromCents(t.AmountCents) : 0m,
                Cleared = t.IsCleared, Archived = t.IsArchived
            }).ToList();
            _grid.DataSource = rows;
            var credit = rows.Sum(r => r.Credit); var debit = rows.Sum(r => r.Debit);
            _summary.Text = $"{rows.Count} registro(s) | Entradas: {credit:C2} | Saídas: {debit:C2} | Movimento líquido: {credit - debit:C2}";
            _reportOptions = new PdfReportExporter.Options(_app.Settings.Get().CompanyName,
                "Consulta avançada de lançamentos", ((AccountFilter)_account.SelectedItem!).Text,
                criteria.StartDate, criteria.EndDate, BuildFilterDescription(), _summary.Text);
        }
        catch (Exception ex) { UiHelpers.ShowError(this, ex); }
        finally { Cursor = Cursors.Default; }
    }

    private void ExportCsv()
    {
        if (_grid.Rows.Count == 0) { MessageBox.Show(this, "Não há dados para exportar."); return; }
        using var dlg = new SaveFileDialog { Filter = "CSV (*.csv)|*.csv", FileName = $"consulta-lancamentos-{DateTime.Now:yyyyMMdd-HHmm}.csv" };
        if (dlg.ShowDialog(this) != DialogResult.OK) return;
        try { CsvExporter.Export(_grid, dlg.FileName); MessageBox.Show(this, "Arquivo exportado."); }
        catch (Exception ex) { UiHelpers.ShowError(this, ex); }
    }

    private void ExportPdf()
    {
        if (_reportOptions is not { } options || _grid.Rows.Count == 0) { MessageBox.Show(this, "Não há dados para gerar o PDF."); return; }
        using var dlg = new SaveFileDialog { Filter = "PDF (*.pdf)|*.pdf", FileName = $"consulta-lancamentos-{DateTime.Now:yyyyMMdd-HHmm}.pdf" };
        if (dlg.ShowDialog(this) != DialogResult.OK) return;
        try
        {
            PdfReportExporter.Export(_grid, dlg.FileName, options);
            PdfReportExporter.OfferOpen(this, dlg.FileName);
        }
        catch (Exception ex) { UiHelpers.ShowError(this, ex); }
    }

    private string BuildFilterDescription()
    {
        var parts = new List<string>();
        var history = (HistoryFilter)_history.SelectedItem!; var cleared = (BoolFilter)_cleared.SelectedItem!; var archived = (BoolFilter)_archived.SelectedItem!;
        if (history.Id.HasValue) parts.Add($"Histórico: {history.Text}");
        if (!string.IsNullOrWhiteSpace(_document.Text)) parts.Add($"Documento contém: {_document.Text.Trim()}");
        if (!string.IsNullOrWhiteSpace(_description.Text)) parts.Add($"Complemento contém: {_description.Text.Trim()}");
        if (cleared.Value.HasValue) parts.Add($"Situação: {cleared.Text}");
        parts.Add($"Baixa: {archived.Text}");
        if (_minAmount.Value > 0) parts.Add($"Valor mínimo: {_minAmount.Value:C2}");
        if (_maxAmount.Value > 0) parts.Add($"Valor máximo: {_maxAmount.Value:C2}");
        return parts.Count == 0 ? "Sem filtros adicionais" : string.Join(" | ", parts);
    }

    private sealed record AccountFilter(long? Id, string Text) { public override string ToString() => Text; }
    private sealed record HistoryFilter(long? Id, string Text) { public override string ToString() => Text; }
    private sealed record BoolFilter(bool? Value, string Text) { public override string ToString() => Text; }
    private sealed class SearchRow
    {
        public string Account { get; init; } = string.Empty;
        public string Date { get; init; } = string.Empty;
        public string Document { get; init; } = string.Empty;
        public string History { get; init; } = string.Empty;
        public string Description { get; init; } = string.Empty;
        public decimal Credit { get; init; }
        public decimal Debit { get; init; }
        public bool Cleared { get; init; }
        public bool Archived { get; init; }
    }
}
