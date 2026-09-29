using System.IO.Compression;
using System.Text;
using Caixa.Core.Data;
using Microsoft.Data.Sqlite;

namespace Caixa.Core.Services;

/// <summary>
/// Cria e restaura backups consistentes do banco SQLite.
/// O backup usa a API de backup do SQLite em vez de copiar diretamente o arquivo aberto.
/// </summary>
public sealed class BackupService
{
    private readonly Database _db;
    private readonly string? _preRestoreBackupDirectory;

    public BackupService(Database db, string? preRestoreBackupDirectory = null)
    {
        _db = db;
        _preRestoreBackupDirectory = preRestoreBackupDirectory;
    }

    public BackupResult CreateCompressed(string destinationFolder)
        => CreateCompressed(destinationFolder, "backup");

    public BackupResult CreateAutomatic(string destinationFolder, string fileLabel)
        => CreateCompressed(destinationFolder, fileLabel);

    public RestoreResult RestoreCompressed(string backupZipPath)
    {
        if (string.IsNullOrWhiteSpace(backupZipPath))
            throw new ArgumentException("Informe o arquivo ZIP que será restaurado.", nameof(backupZipPath));

        if (!File.Exists(backupZipPath))
            throw new FileNotFoundException("O arquivo de backup não foi encontrado.", backupZipPath);

        var tempFolder = CreateTempFolder("restore");
        var snapshotPath = Path.Combine(tempFolder, "caixa.db");
        BackupResult? safetyBackup = null;

        try
        {
            ExtractDatabaseFromZip(backupZipPath, snapshotPath);
            var sourceInfo = ValidateAndDescribeSnapshot(snapshotPath);

            if (sourceInfo.UserVersion > Database.CurrentUserVersion)
            {
                throw new InvalidDataException(
                    $"Este backup usa um esquema mais novo (user_version={sourceInfo.UserVersion}) " +
                    $"do que esta versão do programa suporta (user_version={Database.CurrentUserVersion}).");
            }

            safetyBackup = CreateCompressed(_preRestoreBackupDirectory ?? AppPaths.PreRestoreBackupDirectory, "pre-restauracao");
            RestoreSqliteSnapshot(snapshotPath);
            _db.Initialize();
            var restoredInfo = ValidateLiveDatabase();

            return new RestoreResult(
                backupZipPath,
                safetyBackup.ZipPath,
                restoredInfo.DatabaseSizeBytes,
                restoredInfo.AccountCount,
                restoredInfo.HistoryCount,
                restoredInfo.TransactionCount,
                restoredInfo.QuickCheckResult,
                restoredInfo.UserVersion);
        }
        catch (Exception ex) when (safetyBackup is not null)
        {
            var rollbackPath = Path.Combine(tempFolder, "rollback-caixa.db");
            var rollbackOk = false;
            string? rollbackError = null;

            try
            {
                ExtractDatabaseFromZip(safetyBackup.ZipPath, rollbackPath);
                ValidateAndDescribeSnapshot(rollbackPath);
                RestoreSqliteSnapshot(rollbackPath);
                _db.Initialize();
                ValidateLiveDatabase();
                rollbackOk = true;
            }
            catch (Exception rollbackEx)
            {
                rollbackError = rollbackEx.Message;
            }

            var rollbackText = rollbackOk
                ? "O banco anterior foi restaurado automaticamente a partir do backup preventivo."
                : "Não foi possível confirmar a restauração automática do banco anterior." +
                  (string.IsNullOrWhiteSpace(rollbackError) ? string.Empty : $"\r\nFalha do rollback: {rollbackError}");

            throw new InvalidOperationException(
                "A restauração não foi concluída.\r\n\r\n" + rollbackText +
                $"\r\n\r\nBackup preventivo:\r\n{safetyBackup.ZipPath}" +
                $"\r\n\r\nFalha original: {ex.Message}", ex);
        }
        finally
        {
            TryDeleteDirectory(tempFolder);
        }
    }

    private BackupResult CreateCompressed(string destinationFolder, string fileLabel)
    {
        if (string.IsNullOrWhiteSpace(destinationFolder))
            throw new ArgumentException("Informe uma pasta de destino para o backup.", nameof(destinationFolder));

        Directory.CreateDirectory(destinationFolder);

        var tempFolder = CreateTempFolder("backup");
        var snapshotPath = Path.Combine(tempFolder, "caixa.db");
        var infoPath = Path.Combine(tempFolder, "backup-info.txt");
        var zipPath = CreateUniqueZipPath(destinationFolder, DateTime.Now, fileLabel);

        try
        {
            CreateSqliteSnapshot(snapshotPath);
            var info = ValidateAndDescribeSnapshot(snapshotPath);

            File.WriteAllText(infoPath, BuildInfoText(info), new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));

            using (var zip = ZipFile.Open(zipPath, ZipArchiveMode.Create))
            {
                zip.CreateEntryFromFile(snapshotPath, "caixa.db", CompressionLevel.Optimal);
                zip.CreateEntryFromFile(infoPath, "backup-info.txt", CompressionLevel.Optimal);
            }

            var zipSize = new FileInfo(zipPath).Length;
            return new BackupResult(
                zipPath,
                info.DatabaseSizeBytes,
                zipSize,
                info.AccountCount,
                info.HistoryCount,
                info.TransactionCount,
                info.QuickCheckResult);
        }
        catch
        {
            if (File.Exists(zipPath))
                File.Delete(zipPath);
            throw;
        }
        finally
        {
            TryDeleteDirectory(tempFolder);
        }
    }

    private void CreateSqliteSnapshot(string snapshotPath)
    {
        var destinationCs = new SqliteConnectionStringBuilder
        {
            DataSource = snapshotPath,
            Mode = SqliteOpenMode.ReadWriteCreate,
            Pooling = false
        }.ToString();

        using (var source = _db.OpenConnection())
        using (var destination = new SqliteConnection(destinationCs))
        {
            destination.Open();
            source.BackupDatabase(destination);

            using var cmd = destination.CreateCommand();
            cmd.CommandText = "PRAGMA journal_mode=DELETE;";
            cmd.ExecuteScalar();
        }

        SqliteConnection.ClearAllPools();
    }

    private void RestoreSqliteSnapshot(string snapshotPath)
    {
        SqliteConnection.ClearAllPools();

        var sourceCs = new SqliteConnectionStringBuilder
        {
            DataSource = snapshotPath,
            Mode = SqliteOpenMode.ReadOnly,
            Pooling = false
        }.ToString();

        using (var source = new SqliteConnection(sourceCs))
        using (var destination = _db.OpenConnection())
        {
            source.Open();
            source.BackupDatabase(destination);

            using var checkpoint = destination.CreateCommand();
            checkpoint.CommandText = "PRAGMA wal_checkpoint(TRUNCATE);";
            checkpoint.ExecuteNonQuery();
        }

        SqliteConnection.ClearAllPools();
    }

    private static void ExtractDatabaseFromZip(string zipPath, string destinationDbPath)
    {
        using var zip = ZipFile.OpenRead(zipPath);

        var entries = zip.Entries
            .Where(e => string.Equals(e.FullName.Replace('\\', '/'), "caixa.db", StringComparison.OrdinalIgnoreCase))
            .ToList();

        if (entries.Count == 0)
            throw new InvalidDataException("O ZIP selecionado não contém o arquivo caixa.db.");

        if (entries.Count > 1)
            throw new InvalidDataException("O ZIP contém mais de um arquivo caixa.db e não pode ser restaurado com segurança.");

        var entry = entries[0];
        if (entry.Length <= 0)
            throw new InvalidDataException("O arquivo caixa.db do backup está vazio.");

        Directory.CreateDirectory(Path.GetDirectoryName(destinationDbPath)!);

        using var input = entry.Open();
        using var output = new FileStream(destinationDbPath, FileMode.CreateNew, FileAccess.Write, FileShare.None);
        input.CopyTo(output);
    }

    private static SnapshotInfo ValidateAndDescribeSnapshot(string snapshotPath)
    {
        var cs = new SqliteConnectionStringBuilder
        {
            DataSource = snapshotPath,
            Mode = SqliteOpenMode.ReadOnly,
            Pooling = false
        }.ToString();

        using var cn = new SqliteConnection(cs);
        cn.Open();
        return ValidateAndDescribeConnection(cn, snapshotPath);
    }

    private SnapshotInfo ValidateLiveDatabase()
    {
        using var cn = _db.OpenConnection();
        return ValidateAndDescribeConnection(cn, _db.DatabasePath);
    }

    private static SnapshotInfo ValidateAndDescribeConnection(SqliteConnection cn, string databasePath)
    {
        using var check = cn.CreateCommand();
        check.CommandText = "PRAGMA quick_check;";
        var quickCheck = Convert.ToString(check.ExecuteScalar()) ?? string.Empty;

        if (!string.Equals(quickCheck, "ok", StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException($"O SQLite não validou o banco. PRAGMA quick_check retornou: {quickCheck}");

        EnsureRequiredTables(cn);
        using (var foreignKeys = cn.CreateCommand())
        {
            foreignKeys.CommandText = "PRAGMA foreign_key_check;";
            using var violations = foreignKeys.ExecuteReader();
            if (violations.Read())
                throw new InvalidDataException("O banco contém referências inválidas entre tabelas (foreign_key_check).");
        }

        // Verifica também vínculos em bancos cujo esquema não declarou as FKs.
        using (var references = cn.CreateCommand())
        {
            references.CommandText = """
                SELECT EXISTS(SELECT 1 FROM transactions t
                    LEFT JOIN accounts a ON a.id=t.account_id
                    LEFT JOIN histories h ON h.id=t.history_id
                    WHERE a.id IS NULL OR h.id IS NULL);
                """;
            if (Convert.ToInt64(references.ExecuteScalar()) != 0)
                throw new InvalidDataException("O banco contém lançamentos sem conta ou histórico correspondente.");
        }

        using var versionCmd = cn.CreateCommand();
        versionCmd.CommandText = "PRAGMA user_version;";
        var userVersion = Convert.ToInt32(versionCmd.ExecuteScalar());

        static long Count(SqliteConnection connection, string table)
        {
            using var cmd = connection.CreateCommand();
            cmd.CommandText = $"SELECT COUNT(*) FROM {table};";
            return Convert.ToInt64(cmd.ExecuteScalar());
        }

        return new SnapshotInfo(
            new FileInfo(databasePath).Length,
            Count(cn, "accounts"),
            Count(cn, "histories"),
            Count(cn, "transactions"),
            quickCheck,
            userVersion);
    }

    private static void EnsureRequiredTables(SqliteConnection cn)
    {
        // Colunas usadas pelo programa; colunas adicionais são aceitas.
        var required = new Dictionary<string, string[]>
        {
            ["accounts"] = ["id", "code", "number", "name", "opening_balance_cents", "legacy_stored_real_cents", "legacy_stored_cleared_cents", "is_active", "created_at"],
            ["histories"] = ["id", "code", "description", "direction", "is_active"],
            ["transactions"] = ["id", "account_id", "date", "document_number", "history_id", "amount_cents", "is_cleared", "is_archived", "legacy_rolled_into_opening_balance", "description", "legacy_source", "legacy_record_number", "created_at", "updated_at"],
            ["settings"] = ["key", "value"]
        };

        foreach (var (table, columns) in required)
        {
            using var cmd = cn.CreateCommand();
            cmd.CommandText = "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=$name;";
            cmd.Parameters.AddWithValue("$name", table);
            if (Convert.ToInt32(cmd.ExecuteScalar()) != 1)
                throw new InvalidDataException($"O backup não possui a tabela obrigatória '{table}'.");
            using var schema = cn.CreateCommand();
            schema.CommandText = "SELECT name FROM pragma_table_info($table);";
            schema.Parameters.AddWithValue("$table", table);
            using var reader = schema.ExecuteReader();
            var found = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            while (reader.Read()) found.Add(reader.GetString(0));
            var missing = columns.Where(c => !found.Contains(c)).ToArray();
            if (missing.Length > 0)
                throw new InvalidDataException($"A tabela '{table}' não possui as colunas necessárias: {string.Join(", ", missing)}.");
        }
    }

    private static string BuildInfoText(SnapshotInfo info)
    {
        var now = DateTime.Now;
        return $"""
            CAIXA MODERNIZADO - BACKUP SQLITE
            =================================

            Criado em.....: {now:dd/MM/yyyy HH:mm:ss}
            Formato........: ZIP contendo snapshot SQLite consistente
            Arquivo........: caixa.db
            Esquema........: user_version={info.UserVersion}
            Verificação....: PRAGMA quick_check = {info.QuickCheckResult}

            Conteúdo do banco
            -----------------
            Contas.........: {info.AccountCount}
            Históricos.....: {info.HistoryCount}
            Lançamentos....: {info.TransactionCount}
            Tamanho SQLite.: {FormatBytes(info.DatabaseSizeBytes)}

            Observação
            ----------
            O arquivo caixa.db deste ZIP foi gerado pela API de backup do SQLite.
            Ele não é uma simples cópia do arquivo que estava aberto pela aplicação.
            """;
    }

    private static string CreateUniqueZipPath(string destinationFolder, DateTime timestamp, string fileLabel)
    {
        var safeLabel = string.IsNullOrWhiteSpace(fileLabel) ? "backup" : fileLabel.Trim();
        var baseName = $"CaixaModernizado-{safeLabel}-{timestamp:yyyyMMdd-HHmmss}";
        var candidate = Path.Combine(destinationFolder, baseName + ".zip");
        if (!File.Exists(candidate))
            return candidate;

        for (var i = 1; i <= 99; i++)
        {
            candidate = Path.Combine(destinationFolder, $"{baseName}-{i:00}.zip");
            if (!File.Exists(candidate))
                return candidate;
        }

        throw new IOException("Não foi possível gerar um nome único para o arquivo de backup.");
    }

    private static string CreateTempFolder(string operation)
    {
        var folder = Path.Combine(
            Path.GetTempPath(),
            "CaixaModernizado",
            $"{operation}-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(folder);
        return folder;
    }

    private static void TryDeleteDirectory(string folder)
    {
        try
        {
            SqliteConnection.ClearAllPools();
            if (Directory.Exists(folder))
                Directory.Delete(folder, recursive: true);
        }
        catch
        {
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

    private sealed record SnapshotInfo(
        long DatabaseSizeBytes,
        long AccountCount,
        long HistoryCount,
        long TransactionCount,
        string QuickCheckResult,
        int UserVersion);
}

public sealed record BackupResult(
    string ZipPath,
    long DatabaseSizeBytes,
    long ZipSizeBytes,
    long AccountCount,
    long HistoryCount,
    long TransactionCount,
    string QuickCheckResult);

public sealed record RestoreResult(
    string SourceZipPath,
    string SafetyBackupPath,
    long DatabaseSizeBytes,
    long AccountCount,
    long HistoryCount,
    long TransactionCount,
    string QuickCheckResult,
    int UserVersion);
