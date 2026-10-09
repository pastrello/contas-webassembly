using Caixa.Core;
using Caixa.Core.Models;
using Caixa.Core.Utilities;
using Caixa.WinForms.Utilities;

namespace Caixa.WinForms.Forms;

/// <summary>Lançamentos financeiros com cálculo automático de saldos.</summary>
public partial class TransactionsForm : Form
{
    private readonly CaixaApplication _app;
    private CashTransaction? _loaded;
    private bool _suppressGridSelection;
    private readonly UnsavedChangesGuard _unsaved = new();
    private readonly Func<bool>? _confirmDiscard;

    public TransactionsForm(CaixaApplication app) : this(app, null) { }

    internal TransactionsForm(CaixaApplication app, Func<bool>? confirmDiscard)
    {
        _app = app;
        _confirmDiscard = confirmDiscard;
        InitializeComponent();
        UiHelpers.ConfigureGrid(grid);
        grid.Columns.Add(UiHelpers.TextColumn("Data", nameof(TxRow.Date), 90));
        grid.Columns.Add(UiHelpers.TextColumn("Doc.", nameof(TxRow.Document), 80));
        grid.Columns.Add(UiHelpers.TextColumn("Hist.", nameof(TxRow.History), 180));
        grid.Columns.Add(UiHelpers.TextColumn("Complemento", nameof(TxRow.Description), 300));
        grid.Columns.Add(UiHelpers.MoneyColumn("Valor", nameof(TxRow.Amount), 115));
        grid.Columns.Add(UiHelpers.TextColumn("Tipo", nameof(TxRow.Direction), 70));
        grid.Columns.Add(UiHelpers.CheckColumn("Contab.", nameof(TxRow.Cleared), 70));
        grid.Columns.Add(UiHelpers.CheckColumn("Baixado", nameof(TxRow.Archived), 70));

        cmbAccount.SelectedIndexChanged += (_, _) => { ReloadTransactions(); ResetEditor(); };
        chkShowArchived.CheckedChanged += (_, _) => { ReloadTransactions(); ResetEditor(); };
        Load += (_, _) => LoadCombos();
        FormClosing += (_, e) => { if (!_unsaved.ConfirmClose(this, CaptureState())) e.Cancel = true; };
        date.ValueChanged += (_, _) => UpdateToggleEnabled();
        txtDocument.TextChanged += (_, _) => UpdateToggleEnabled();
        cmbHistory.SelectedIndexChanged += (_, _) => UpdateToggleEnabled();
        numAmount.ValueChanged += (_, _) => UpdateToggleEnabled();
        txtDescription.TextChanged += (_, _) => UpdateToggleEnabled();
        chkCleared.CheckedChanged += (_, _) => UpdateToggleEnabled();
    }

    private Account? CurrentAccount => cmbAccount.SelectedItem as Account;
    private HistoryItem? CurrentHistory => cmbHistory.SelectedItem as HistoryItem;

    private void LoadCombos()
    {
        cmbAccount.DataSource = _app.Accounts.GetAll(activeOnly: true);
        cmbHistory.DataSource = _app.Histories.GetAll(activeOnly: true);
        if (cmbAccount.Items.Count > 0) cmbAccount.SelectedIndex = 0;
        if (cmbHistory.Items.Count > 0) cmbHistory.SelectedIndex = 0;
        ReloadTransactions();
        NewItem();
    }

    private void ReloadTransactions(long? selectId = null)
    {
        var account = CurrentAccount;
        if (account is null)
        {
            _suppressGridSelection = true;
            try { grid.DataSource = null; }
            finally { _suppressGridSelection = false; }
            lblReal.Text = lblCleared.Text = "R$ 0,00";
            lblPending.Text = "R$ 0,00";
            return;
        }

        var rows = _app.Transactions.GetByAccount(account.Id, chkShowArchived.Checked)
            .Select(t => new TxRow
            {
                Id = t.Id,
                Date = t.Date.ToString("dd/MM/yyyy"),
                Document = t.DocumentNumber,
                History = $"{t.HistoryCode} - {t.HistoryDescription}",
                Description = t.Description,
                Amount = Money.FromCents(t.AmountCents),
                Direction = t.Direction >= 0 ? "Entrada" : "Saída",
                Cleared = t.IsCleared,
                Archived = t.IsArchived
            }).ToList();

        _suppressGridSelection = true;
        try
        {
            grid.DataSource = rows;
            if (selectId.HasValue)
            {
                foreach (DataGridViewRow row in grid.Rows)
                {
                    if ((row.DataBoundItem as TxRow)?.Id != selectId.Value) continue;
                    row.Selected = true;
                    grid.CurrentCell = row.Cells[0];
                    break;
                }
            }
        }
        finally
        {
            _suppressGridSelection = false;
        }

        var b = _app.Balances.Calculate(account.Id);
        lblReal.Text = Money.Format(b.RealBalanceCents);
        lblCleared.Text = Money.Format(b.ClearedBalanceCents);
        lblPending.Text = Money.Format(b.PendingNetCents);
    }

    private void NewItem()
    {
        if (_unsaved.IsDirty(CaptureState()) &&
            !(_confirmDiscard?.Invoke() ?? _unsaved.ConfirmDiscard(this, CaptureState()))) return;
        ResetEditor();
    }

    private void ResetEditor()
    {
        _suppressGridSelection = true;
        try
        {
            _loaded = null;
            grid.CurrentCell = null;
            grid.ClearSelection();
            date.Value = DateTime.Today;
            txtDocument.Clear();
            numAmount.Value = 0;
            txtDescription.Clear();
            chkCleared.Checked = false;
            SetEditorEnabled(true);
            cmbHistory.DataSource = _app.Histories.GetAll(activeOnly: true);
            if (cmbHistory.Items.Count > 0) cmbHistory.SelectedIndex = 0;
        }
        finally
        {
            _suppressGridSelection = false;
        }

        _unsaved.Accept(CaptureState());
        UpdateModeTitle();
        date.Focus();
    }

    private void SelectCurrent()
    {
        if (_suppressGridSelection) return;
        if (grid.CurrentRow?.DataBoundItem is not TxRow row) return;
        var account = CurrentAccount; if (account is null) return;
        var tx = _app.Transactions.GetByAccount(account.Id, includeArchived: true).FirstOrDefault(t => t.Id == row.Id);
        if (tx is null) return;
        _loaded = tx;
        date.Value = tx.Date.ToDateTime(TimeOnly.MinValue);
        txtDocument.Text = tx.DocumentNumber;
        // Mantém o histórico original disponível para editar movimentos antigos,
        // mesmo quando ele deixou de estar ativo para novos lançamentos.
        var histories = _app.Histories.GetAll(activeOnly: true);
        if (histories.All(h => h.Id != tx.HistoryId))
        {
            var original = _app.Histories.GetById(tx.HistoryId);
            if (original is not null) histories.Add(original);
        }
        cmbHistory.DataSource = histories;
        cmbHistory.SelectedItem = histories.FirstOrDefault(h => h.Id == tx.HistoryId);
        numAmount.Value = Math.Min(numAmount.Maximum, Math.Max(numAmount.Minimum, Money.FromCents(tx.AmountCents)));
        txtDescription.Text = tx.Description;
        chkCleared.Checked = tx.IsCleared;
        SetEditorEnabled(!tx.LegacyRolledIntoOpeningBalance);
        lblLegacy.Visible = tx.LegacyRolledIntoOpeningBalance;
        _unsaved.Accept(CaptureState());
        UpdateModeTitle();
    }

    private bool SaveItem()
    {
        var account = CurrentAccount; var history = CurrentHistory;
        if (account is null || history is null) return false;
        if (_loaded?.LegacyRolledIntoOpeningBalance == true)
        {
            MessageBox.Show(this, "Este registro está protegido por compatibilidade com dados existentes e é somente leitura.", "Somente leitura",
                MessageBoxButtons.OK, MessageBoxIcon.Information);
            return false;
        }
        try
        {
            // Só substitui o retrato carregado depois que a gravação for bem-sucedida.
            var tx = CaptureTransaction();
            var id = _app.Transactions.Save(tx);
            _loaded = tx;
            ReloadTransactions(id);
            _unsaved.Accept(CaptureState());
            UpdateModeTitle();
            return true;
        }
        catch (Exception ex)
        {
            UiHelpers.ShowError(this, ex);
            return false;
        }
    }

    private void DeleteItem()
    {
        if (_loaded is null) return;
        if (_loaded.LegacyRolledIntoOpeningBalance)
        {
            MessageBox.Show(this, "Este registro está protegido e não pode ser removido pela interface.", "Somente leitura");
            return;
        }
        if (MessageBox.Show(this, "Excluir este lançamento?", "Confirmação", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
        try { _app.Transactions.Delete(_loaded.Id); ResetEditor(); ReloadTransactions(); }
        catch (Exception ex) { UiHelpers.ShowError(this, ex); }
    }

    private void ToggleCleared()
    {
        if (_loaded is null || _loaded.LegacyRolledIntoOpeningBalance) return;
        if (_unsaved.IsDirty(CaptureState()))
        {
            MessageBox.Show(this, "Salve as alterações do lançamento antes de contabilizar ou desfazer a contabilização.",
                "Alterações não salvas", MessageBoxButtons.OK, MessageBoxIcon.Information);
            return;
        }
        try
        {
            _app.Transactions.SetCleared(_loaded.Id, !_loaded.IsCleared);
            _loaded.IsCleared = !_loaded.IsCleared;
            chkCleared.Checked = _loaded.IsCleared;
            ReloadTransactions(_loaded.Id);
            _unsaved.Accept(CaptureState());
            UpdateModeTitle();
        }
        catch (Exception ex) { UiHelpers.ShowError(this, ex); }
    }

    private string CaptureState()
        => string.Join("\u001F", _loaded?.Id ?? 0, CurrentAccount?.Id ?? 0, date.Value.Date, txtDocument.Text,
            CurrentHistory?.Id ?? 0, numAmount.Value, txtDescription.Text, chkCleared.Checked);

    private CashTransaction CaptureTransaction() => new()
    {
        Id = _loaded?.Id ?? 0,
        AccountId = CurrentAccount?.Id ?? 0,
        HistoryId = CurrentHistory?.Id ?? 0,
        Date = DateOnly.FromDateTime(date.Value.Date),
        DocumentNumber = txtDocument.Text,
        AmountCents = Money.ToCents(numAmount.Value),
        Description = txtDescription.Text,
        IsCleared = chkCleared.Checked,
        IsArchived = _loaded?.IsArchived ?? false,
        LegacyRolledIntoOpeningBalance = _loaded?.LegacyRolledIntoOpeningBalance ?? false
    };

    private void UpdateModeTitle()
    {
        UpdateToggleEnabled();
    }

    private void UpdateToggleEnabled()
    {
        var dirty = _unsaved.IsDirty(CaptureState());
        Text = (_loaded is null ? "Lançamentos - NOVO REGISTRO" : $"Lançamentos - EDITANDO #{_loaded.Id}")
            + (dirty ? " - ALTERAÇÕES NÃO SALVAS (Salvar ou Novo)" : "");
        btnToggle.Enabled = _loaded is { LegacyRolledIntoOpeningBalance: false } && !dirty;
        // A conta é o contexto da grade. Navegar só após Salvar ou descartar por Novo.
        grid.Enabled = cmbAccount.Enabled = chkShowArchived.Enabled = !dirty;
    }

    private void SetEditorEnabled(bool enabled)
    {
        date.Enabled = enabled; txtDocument.ReadOnly = !enabled; cmbHistory.Enabled = enabled; numAmount.Enabled = enabled;
        txtDescription.ReadOnly = !enabled; chkCleared.Enabled = enabled; btnSave.Enabled = enabled; btnDelete.Enabled = enabled; btnToggle.Enabled = enabled;
        if (enabled) lblLegacy.Visible = false;
    }

    private sealed class TxRow
    {
        public long Id { get; init; }
        public string Date { get; init; } = string.Empty;
        public string Document { get; init; } = string.Empty;
        public string History { get; init; } = string.Empty;
        public string Description { get; init; } = string.Empty;
        public decimal Amount { get; init; }
        public string Direction { get; init; } = string.Empty;
        public bool Cleared { get; init; }
        public bool Archived { get; init; }
    }
}
