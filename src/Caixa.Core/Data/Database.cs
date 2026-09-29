using Microsoft.Data.Sqlite;

namespace Caixa.Core.Data;

/// <summary>Inicializa e abre o banco SQLite da aplicação.</summary>
public sealed class Database
{
    public const int CurrentUserVersion = 1;
    private readonly string _databasePath;

    public Database(string? databasePath = null)
    {
        _databasePath = databasePath ?? AppPaths.DatabasePath;
    }

    public string DatabasePath => _databasePath;

    public SqliteConnection OpenConnection()
    {
        Directory.CreateDirectory(Path.GetDirectoryName(_databasePath)!);
        var cs = new SqliteConnectionStringBuilder
        {
            DataSource = _databasePath,
            Mode = SqliteOpenMode.ReadWriteCreate
        }.ToString();
        var connection = new SqliteConnection(cs);
        try
        {
            connection.Open();
            using var cmd = connection.CreateCommand();
            cmd.CommandText = "PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;";
            cmd.ExecuteNonQuery();
            return connection;
        }
        catch
        {
            connection.Dispose();
            throw;
        }
    }

    public void Initialize()
    {
        using var cn = OpenConnection();
        using (var version = cn.CreateCommand())
        {
            version.CommandText = "PRAGMA user_version;";
            var current = Convert.ToInt32(version.ExecuteScalar());
            if (current > CurrentUserVersion)
                throw new InvalidDataException($"O banco usa um esquema mais novo ({current}) que o suportado ({CurrentUserVersion}). Atualize o programa.");
        }
        using var cmd = cn.CreateCommand();
        cmd.CommandText = """
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;

            CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                code TEXT NOT NULL UNIQUE,
                number TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                opening_balance_cents INTEGER NOT NULL DEFAULT 0,
                legacy_stored_real_cents INTEGER NULL,
                legacy_stored_cleared_cents INTEGER NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS histories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                code TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL,
                direction INTEGER NOT NULL CHECK(direction IN (-1,1)),
                is_active INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS transactions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL,
                date TEXT NOT NULL,
                document_number TEXT NOT NULL DEFAULT '',
                history_id INTEGER NOT NULL,
                amount_cents INTEGER NOT NULL CHECK(amount_cents >= 0),
                is_cleared INTEGER NOT NULL DEFAULT 0,
                is_archived INTEGER NOT NULL DEFAULT 0,
                legacy_rolled_into_opening_balance INTEGER NOT NULL DEFAULT 0,
                description TEXT NOT NULL DEFAULT '',
                legacy_source TEXT NULL,
                legacy_record_number INTEGER NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE RESTRICT,
                FOREIGN KEY(history_id) REFERENCES histories(id) ON DELETE RESTRICT
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS ix_transactions_account_date
                ON transactions(account_id, date, id);
            CREATE INDEX IF NOT EXISTS ix_transactions_account_document
                ON transactions(account_id, document_number);
            CREATE INDEX IF NOT EXISTS ix_transactions_history_date
                ON transactions(history_id, date);
            CREATE INDEX IF NOT EXISTS ix_transactions_cleared_archived
                ON transactions(account_id, is_cleared, is_archived, date);

            INSERT OR IGNORE INTO settings(key, value) VALUES ('CompanyName', 'CONTROLE BANCÁRIO');
            PRAGMA user_version=1;
            """;
        cmd.ExecuteNonQuery();
    }

    public bool HasAccounts()
    {
        using var cn = OpenConnection();
        using var cmd = cn.CreateCommand();
        cmd.CommandText = "SELECT EXISTS(SELECT 1 FROM accounts LIMIT 1);";
        return Convert.ToInt32(cmd.ExecuteScalar()) == 1;
    }
}
