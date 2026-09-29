using System.IO.Compression;
using System.Reflection;
using Caixa.Core;
using Caixa.Core.Data;
using Caixa.Core.Models;
using Caixa.Core.Services;
using Caixa.WinForms.Forms;
using Microsoft.Data.Sqlite;

// Executável de regressão sem dependências adicionais e sem abrir janelas.
// Todos os bancos e backups ficam sob o diretório informado na linha de comando.
internal static class Program
{
    private static int _checks;

    [STAThread]
    private static int Main(string[] args)
    {
        if (args.Length != 1) throw new ArgumentException("Informe o diretório para os dados de teste.");
        var root = Path.Combine(Path.GetFullPath(args[0]), Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try
        {
            Run(root);
            Console.WriteLine($"PASS: {_checks} verificações. Dados descartáveis: {root}");
            return 0;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine(ex);
            return 1;
        }
        finally { SqliteConnection.ClearAllPools(); }
    }

    private static void Run(string root)
    {
        var app = new CaixaApplication(Path.Combine(root, "caixa.db"));
        var account = new Account { Code = "01", Name = "Teste", OpeningBalanceCents = 10000 };
        app.Accounts.Save(account);
        var history = new HistoryItem { Code = "01", Description = "Entrada", Direction = 1 };
        app.Histories.Save(history);
        var date = new DateOnly(2026, 1, 10);
        var movement = new CashTransaction { AccountId = account.Id, HistoryId = history.Id, Date = date, AmountCents = 2500, IsCleared = true };
        app.Transactions.Save(movement);
        history.Direction = -1;
        Reject<InvalidOperationException>(() => app.Histories.Save(history), "direção de histórico utilizado");
        Equal(12500L, app.Balances.Calculate(account.Id).RealBalanceCents, "saldo preservado após rejeição");
        Equal(1, app.Histories.GetById(history.Id)!.Direction, "direção persistida preservada");
        history.Direction = 1;
        history.Description = "Descrição alterada";
        history.IsActive = false;
        app.Histories.Save(history);
        Equal(false, app.Histories.GetById(history.Id)!.IsActive, "desativação permitida");
        var unused = new HistoryItem { Code = "02", Description = "Sem movimentos" };
        app.Histories.Save(unused);
        unused.Direction = -1;
        app.Histories.Save(unused);
        Equal(-1, app.Histories.GetById(unused.Id)!.Direction, "direção de histórico sem movimentos");
        app.Transactions.ArchiveCleared(account.Id, date, date);
        history.Direction = -1;
        Reject<InvalidOperationException>(() => app.Histories.Save(history), "direção com movimentos arquivados");
        Equal(12500L, app.Balances.CalculateBefore(account.Id, date.AddDays(1)), "saldo anterior inclui arquivados");
        Equal(0, app.Reports.Statement(account.Id, date.AddDays(1), date.AddDays(2)).Count, "período vazio");

        // Simula um registro já incorporado ao saldo inicial de um banco existente.
        Sql(app.Database, "UPDATE transactions SET legacy_rolled_into_opening_balance=1;");
        Equal(10000L, app.Balances.CalculateBefore(account.Id, date.AddDays(1)), "compatibilidade evita dupla contagem");
        Sql(app.Database, "UPDATE transactions SET legacy_rolled_into_opening_balance=0, is_archived=0;");

        var backup = new BackupService(app.Database, Path.Combine(root, "preventivos"));
        var zip = backup.CreateCompressed(Path.Combine(root, "backups"));
        Equal("ok", zip.QuickCheckResult, "backup validado");
        movement.AmountCents = 7000;
        app.Transactions.Save(movement);
        var restored = backup.RestoreCompressed(zip.ZipPath);
        Equal(true, File.Exists(restored.SafetyBackupPath), "backup preventivo criado");
        Equal(12500L, app.Balances.Calculate(account.Id).RealBalanceCents, "restauração recupera saldo");
        Equal("wal", Scalar(app.Database, "PRAGMA journal_mode;"), "WAL após restauração");

        TestInvalidBackup(root, zip.ZipPath, backup, app, "coluna", "ALTER TABLE accounts DROP COLUMN number;");
        TestInvalidBackup(root, zip.ZipPath, backup, app, "orfao", "PRAGMA foreign_keys=OFF; UPDATE transactions SET account_id=9999;");
        TestInvalidBackup(root, zip.ZipPath, backup, app, "versao", "PRAGMA user_version=999;");

        var newer = new Database(Path.Combine(root, "novo.db"));
        newer.Initialize();
        Sql(newer, "PRAGMA user_version=999;");
        Reject<InvalidDataException>(newer.Initialize, "abertura de esquema futuro");
        Equal("999", Scalar(newer, "PRAGMA user_version;"), "versão futura não sobrescrita");

        TestForms(app, account, history, date);
    }

    private static void TestForms(CaixaApplication app, Account account, HistoryItem history, DateOnly date)
    {
        using var transactions = new TransactionsForm(app);
        transactions.BindingContext = new BindingContext();
        Call(transactions, "LoadCombos");
        var grid = Field<DataGridView>(transactions, "grid");
        grid.CurrentCell = grid.Rows[0].Cells[0];
        Call(transactions, "SelectCurrent");
        Equal(history.Id, ((HistoryItem)Field<ComboBox>(transactions, "cmbHistory").SelectedItem!).Id, "edição mantém histórico inativo");
        Equal(true, Field<Button>(transactions, "btnToggle").Enabled, "contabilização disponível sem alterações");
        Field<TextBox>(transactions, "txtDescription").Text = "Editado";
        Equal(false, Field<Button>(transactions, "btnToggle").Enabled, "contabilização bloqueada enquanto há edições");
        Equal("", app.Transactions.GetByAccount(account.Id)[0].Description, "edições ainda não persistidas");
        Equal(true, (bool)Call(transactions, "SaveItem")!, "salvar com histórico inativo");
        Equal("Editado", app.Transactions.GetByAccount(account.Id)[0].Description, "complemento persistido");
        Equal(true, Field<Button>(transactions, "btnToggle").Enabled, "contabilização liberada após salvar");
        Call(transactions, "ToggleCleared");
        Equal(false, app.Transactions.GetByAccount(account.Id)[0].IsCleared, "descontabilização persistida");
        Equal("Editado", app.Transactions.GetByAccount(account.Id)[0].Description, "contabilização preserva complemento");
        Call(transactions, "NewItem");
        Equal(false, ((IEnumerable<HistoryItem>)Field<ComboBox>(transactions, "cmbHistory").DataSource!).Any(h => h.Id == history.Id), "novo registro exclui histórico inativo");
        Equal(false, Field<Button>(transactions, "btnToggle").Enabled, "novo registro não permite contabilização avulsa");

        using var report = new PeriodReportForm(app, PeriodReportMode.Statement);
        Field<ComboBox>(report, "_account").DataSource = app.Accounts.GetAll();
        Field<DateTimePicker>(report, "_start").Value = date.AddDays(1).ToDateTime(TimeOnly.MinValue);
        Field<DateTimePicker>(report, "_end").Value = date.AddDays(2).ToDateTime(TimeOnly.MinValue);
        Call(report, "LoadReport");
        Equal(true, Field<Label>(report, "_summary").Text.Contains(125m.ToString("C2")), "extrato vazio mostra saldo anterior");
        var options = Field<object>(report, "_reportOptions");
        Field<DateTimePicker>(report, "_start").Value = DateTime.Today;
        Equal(date.AddDays(1), Property<DateOnly>(options, "StartDate"), "PDF mantém período consultado");

        using var search = new TransactionSearchForm(app);
        Call(search, "LoadFilters");
        Field<CheckBox>(search, "_usePeriod").Checked = false;
        Call(search, "Search");
        var searchOptions = Field<object>(search, "_reportOptions");
        Field<TextBox>(search, "_document").Text = "filtro ainda não consultado";
        Equal(false, Property<string>(searchOptions, "FilterDescription").Contains("filtro ainda"), "PDF mantém filtros consultados");

        // Usa um histórico ativo para o relatório seletivo.
        var savedHistory = app.Histories.GetById(history.Id)!;
        savedHistory.IsActive = true;
        app.Histories.Save(savedHistory);
        using var histories = new HistoryReportForm(app);
        Call(histories, "LoadData");
        Field<DateTimePicker>(histories, "_start").Value = date.ToDateTime(TimeOnly.MinValue);
        Field<DateTimePicker>(histories, "_end").Value = date.ToDateTime(TimeOnly.MinValue);
        Field<CheckedListBox>(histories, "_histories").SetItemChecked(0, true);
        Call(histories, "Query");
        var historyOptions = Field<object>(histories, "_reportOptions");
        Field<CheckedListBox>(histories, "_histories").SetItemChecked(0, false);
        Equal(true, Property<string>(historyOptions, "FilterDescription").Contains(savedHistory.Description), "PDF mantém históricos consultados");
    }

    private static void TestInvalidBackup(string root, string validZip, BackupService backup, CaixaApplication app, string name, string sql)
    {
        var folder = Path.Combine(root, name);
        Directory.CreateDirectory(folder);
        using (var archive = ZipFile.OpenRead(validZip)) archive.GetEntry("caixa.db")!.ExtractToFile(Path.Combine(folder, "caixa.db"));
        var db = new Database(Path.Combine(folder, "caixa.db"));
        Sql(db, sql);
        SqliteConnection.ClearAllPools();
        var path = Path.Combine(root, name + ".zip");
        ZipFile.CreateFromDirectory(folder, path);
        Reject<InvalidDataException>(() => backup.RestoreCompressed(path), "backup inválido: " + name);
        Equal(12500L, app.Balances.Calculate(app.Accounts.GetAll()[0].Id).RealBalanceCents, "rejeição preserva banco: " + name);
    }

    private static void Sql(Database db, string sql)
    {
        using var connection = db.OpenConnection();
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }

    private static string Scalar(Database db, string sql)
    {
        using var connection = db.OpenConnection();
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        return Convert.ToString(command.ExecuteScalar())!;
    }

    private static T Field<T>(object instance, string name) => (T)instance.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(instance)!;
    private static T Property<T>(object instance, string name) => (T)instance.GetType().GetProperty(name)!.GetValue(instance)!;
    private static object? Call(object instance, string name) => instance.GetType().GetMethod(name, BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(instance, null);
    private static void Equal<T>(T expected, T actual, string name)
    {
        if (!EqualityComparer<T>.Default.Equals(expected, actual)) throw new Exception($"FAIL {name}: esperado {expected}, obtido {actual}");
        _checks++;
    }
    private static void Reject<T>(Action action, string name) where T : Exception
    {
        try { action(); }
        catch (T) { _checks++; return; }
        throw new Exception("FAIL operação deveria ser rejeitada: " + name);
    }
}
