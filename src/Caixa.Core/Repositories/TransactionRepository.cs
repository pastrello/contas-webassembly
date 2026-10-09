using Caixa.Core.Data;
using Caixa.Core.Models;
using Microsoft.Data.Sqlite;

namespace Caixa.Core.Repositories;

/// <summary>Persistência dos lançamentos financeiros.</summary>
public sealed class TransactionRepository
{
    private readonly Database _db;
    public TransactionRepository(Database db) => _db = db;

    public List<CashTransaction> GetByAccount(long accountId, bool includeArchived = false)
    {
        using var cn = _db.OpenConnection();
        using var cmd = cn.CreateCommand();
        cmd.CommandText = BaseSelect + "\n" + """
            WHERE t.account_id=$accountId AND ($includeArchived=1 OR t.is_archived=0)
            ORDER BY t.date DESC, t.id DESC;
            """;
        cmd.Parameters.AddWithValue("$accountId", accountId);
        cmd.Parameters.AddWithValue("$includeArchived", includeArchived ? 1 : 0);
        return ReadList(cmd);
    }

    public List<CashTransaction> GetByPeriod(long accountId, DateOnly start, DateOnly end,
        bool? cleared = null, bool? archived = null, IReadOnlyCollection<long>? historyIds = null)
    {
        using var cn = _db.OpenConnection();
        using var cmd = cn.CreateCommand();
        var sql = BaseSelect + " WHERE t.account_id=$accountId AND t.date BETWEEN $start AND $end";
        cmd.Parameters.AddWithValue("$accountId", accountId);
        cmd.Parameters.AddWithValue("$start", start.ToString("yyyy-MM-dd"));
        cmd.Parameters.AddWithValue("$end", end.ToString("yyyy-MM-dd"));
        if (cleared.HasValue)
        {
            sql += " AND t.is_cleared=$cleared";
            cmd.Parameters.AddWithValue("$cleared", cleared.Value ? 1 : 0);
        }
        if (archived.HasValue)
        {
            sql += " AND t.is_archived=$archived";
            cmd.Parameters.AddWithValue("$archived", archived.Value ? 1 : 0);
        }
        if (historyIds is { Count: > 0 })
        {
            var names = new List<string>();
            var i = 0;
            foreach (var id in historyIds)
            {
                var p = "$h" + i++;
                names.Add(p);
                cmd.Parameters.AddWithValue(p, id);
            }
            sql += $" AND t.history_id IN ({string.Join(',', names)})";
        }
        sql += " ORDER BY t.date, t.id;";
        cmd.CommandText = sql;
        return ReadList(cmd);
    }

    public List<CashTransaction> Search(TransactionSearchCriteria criteria)
    {
        ArgumentNullException.ThrowIfNull(criteria);

        if (criteria.StartDate.HasValue && criteria.EndDate.HasValue && criteria.EndDate < criteria.StartDate)
            throw new InvalidOperationException("A data final deve ser igual ou posterior à inicial.");
        if (criteria.MinimumAmountCents.HasValue && criteria.MaximumAmountCents.HasValue &&
            criteria.MaximumAmountCents < criteria.MinimumAmountCents)
            throw new InvalidOperationException("O valor máximo deve ser igual ou maior que o mínimo.");

        using var cn = _db.OpenConnection();
        using var cmd = cn.CreateCommand();
        var sql = BaseSelect + "\nWHERE 1=1";

        if (criteria.AccountId.HasValue)
        {
            sql += " AND t.account_id=$accountId";
            cmd.Parameters.AddWithValue("$accountId", criteria.AccountId.Value);
        }
        if (criteria.HistoryId.HasValue)
        {
            sql += " AND t.history_id=$historyId";
            cmd.Parameters.AddWithValue("$historyId", criteria.HistoryId.Value);
        }
        if (criteria.StartDate.HasValue)
        {
            sql += " AND t.date >= $start";
            cmd.Parameters.AddWithValue("$start", criteria.StartDate.Value.ToString("yyyy-MM-dd"));
        }
        if (criteria.EndDate.HasValue)
        {
            sql += " AND t.date <= $end";
            cmd.Parameters.AddWithValue("$end", criteria.EndDate.Value.ToString("yyyy-MM-dd"));
        }
        if (!string.IsNullOrWhiteSpace(criteria.DocumentContains))
        {
            sql += " AND t.document_number LIKE $document COLLATE NOCASE";
            cmd.Parameters.AddWithValue("$document", $"%{criteria.DocumentContains.Trim()}%");
        }
        if (!string.IsNullOrWhiteSpace(criteria.DescriptionContains))
        {
            sql += " AND t.description LIKE $description COLLATE NOCASE";
            cmd.Parameters.AddWithValue("$description", $"%{criteria.DescriptionContains.Trim()}%");
        }
        if (criteria.Cleared.HasValue)
        {
            sql += " AND t.is_cleared=$cleared";
            cmd.Parameters.AddWithValue("$cleared", criteria.Cleared.Value ? 1 : 0);
        }
        if (criteria.Archived.HasValue)
        {
            sql += " AND t.is_archived=$archived";
            cmd.Parameters.AddWithValue("$archived", criteria.Archived.Value ? 1 : 0);
        }
        if (criteria.MinimumAmountCents.HasValue)
        {
            sql += " AND t.amount_cents >= $minAmount";
            cmd.Parameters.AddWithValue("$minAmount", criteria.MinimumAmountCents.Value);
        }
        if (criteria.MaximumAmountCents.HasValue)
        {
            sql += " AND t.amount_cents <= $maxAmount";
            cmd.Parameters.AddWithValue("$maxAmount", criteria.MaximumAmountCents.Value);
        }
        if (!criteria.IncludeLegacyRolled)
            sql += " AND t.legacy_rolled_into_opening_balance=0";

        sql += " ORDER BY t.date DESC, t.id DESC;";
        cmd.CommandText = sql;
        return ReadList(cmd);
    }

    public List<CashTransaction> GetIncludedBefore(long accountId, DateOnly date)
    {
        using var cn = _db.OpenConnection();
        using var cmd = cn.CreateCommand();
        cmd.CommandText = BaseSelect + "\n" + """
            WHERE t.account_id=$accountId
              AND t.date < $date
              AND t.legacy_rolled_into_opening_balance=0
            ORDER BY t.date, t.id;
            """;
        cmd.Parameters.AddWithValue("$accountId", accountId);
        cmd.Parameters.AddWithValue("$date", date.ToString("yyyy-MM-dd"));
        return ReadList(cmd);
    }

    public List<CashTransaction> GetAllIncluded(long accountId)
    {
        using var cn = _db.OpenConnection();
        using var cmd = cn.CreateCommand();
        cmd.CommandText = BaseSelect + "\n" + """
            WHERE t.account_id=$accountId
              AND t.legacy_rolled_into_opening_balance=0
            ORDER BY t.date, t.id;
            """;
        cmd.Parameters.AddWithValue("$accountId", accountId);
        return ReadList(cmd);
    }

    public long Save(CashTransaction item)
    {
        if (item.LegacyRolledIntoOpeningBalance)
            throw new InvalidOperationException("Registros incorporados ao saldo inicial são somente leitura.");
        if (item.AccountId <= 0) throw new InvalidOperationException("Selecione uma conta.");
        if (item.HistoryId <= 0) throw new InvalidOperationException("Selecione um histórico.");
        if (item.AmountCents <= 0) throw new InvalidOperationException("O valor deve ser maior que zero.");

        using var cn = _db.OpenConnection();
        using var cmd = cn.CreateCommand();
        if (item.Id == 0)
        {
            cmd.CommandText = """
                INSERT INTO transactions(account_id,date,document_number,history_id,amount_cents,
                    is_cleared,is_archived,legacy_rolled_into_opening_balance,description)
                VALUES($account,$date,$doc,$history,$amount,$cleared,$archived,0,$description);
                SELECT last_insert_rowid();
                """;
        }
        else
        {
            cmd.CommandText = """
                UPDATE transactions SET account_id=$account,date=$date,document_number=$doc,
                    history_id=$history,amount_cents=$amount,is_cleared=$cleared,
                    is_archived=$archived,description=$description,updated_at=CURRENT_TIMESTAMP
                WHERE id=$id AND legacy_rolled_into_opening_balance=0
                RETURNING id;
                """;
            cmd.Parameters.AddWithValue("$id", item.Id);
        }
        cmd.Parameters.AddWithValue("$account", item.AccountId);
        cmd.Parameters.AddWithValue("$date", item.Date.ToString("yyyy-MM-dd"));
        cmd.Parameters.AddWithValue("$doc", item.DocumentNumber.Trim());
        cmd.Parameters.AddWithValue("$history", item.HistoryId);
        cmd.Parameters.AddWithValue("$amount", item.AmountCents);
        cmd.Parameters.AddWithValue("$cleared", item.IsCleared ? 1 : 0);
        cmd.Parameters.AddWithValue("$archived", item.IsArchived ? 1 : 0);
        cmd.Parameters.AddWithValue("$description", item.Description.Trim());
        var result = cmd.ExecuteScalar();
        if (result is null)
            throw new InvalidOperationException("O lançamento não existe mais ou está protegido por compatibilidade. Atualize a consulta.");
        item.Id = Convert.ToInt64(result);
        return item.Id;
    }

    public void Delete(long id)
    {
        using var cn = _db.OpenConnection();
        using var cmd = cn.CreateCommand();
        cmd.CommandText = "DELETE FROM transactions WHERE id=$id AND legacy_rolled_into_opening_balance=0;";
        cmd.Parameters.AddWithValue("$id", id);
        EnsureChanged(cmd.ExecuteNonQuery());
    }

    public void SetCleared(long id, bool cleared)
    {
        using var cn = _db.OpenConnection();
        using var cmd = cn.CreateCommand();
        cmd.CommandText = "UPDATE transactions SET is_cleared=$v,updated_at=CURRENT_TIMESTAMP WHERE id=$id AND legacy_rolled_into_opening_balance=0;";
        cmd.Parameters.AddWithValue("$v", cleared ? 1 : 0);
        cmd.Parameters.AddWithValue("$id", id);
        EnsureChanged(cmd.ExecuteNonQuery());
    }

    public int ArchiveCleared(long accountId, DateOnly start, DateOnly end)
    {
        if (end < start) throw new InvalidOperationException("A data final deve ser igual ou posterior à inicial.");
        using var cn = _db.OpenConnection();
        using var cmd = cn.CreateCommand();
        cmd.CommandText = """
            UPDATE transactions
            SET is_archived=1, updated_at=CURRENT_TIMESTAMP
            WHERE account_id=$account AND date BETWEEN $start AND $end
              AND is_cleared=1 AND is_archived=0 AND legacy_rolled_into_opening_balance=0;
            """;
        cmd.Parameters.AddWithValue("$account", accountId);
        cmd.Parameters.AddWithValue("$start", start.ToString("yyyy-MM-dd"));
        cmd.Parameters.AddWithValue("$end", end.ToString("yyyy-MM-dd"));
        return cmd.ExecuteNonQuery();
    }

    public int Unarchive(long id)
    {
        using var cn = _db.OpenConnection();
        using var cmd = cn.CreateCommand();
        cmd.CommandText = "UPDATE transactions SET is_archived=0,updated_at=CURRENT_TIMESTAMP WHERE id=$id AND legacy_rolled_into_opening_balance=0;";
        cmd.Parameters.AddWithValue("$id", id);
        var changed = cmd.ExecuteNonQuery();
        EnsureChanged(changed);
        return changed;
    }

    private static void EnsureChanged(int changed)
    {
        if (changed != 1)
            throw new InvalidOperationException("O lançamento não existe mais ou está protegido por compatibilidade. Atualize a consulta.");
    }

    private static List<CashTransaction> ReadList(SqliteCommand cmd)
    {
        using var rd = cmd.ExecuteReader();
        var list = new List<CashTransaction>();
        while (rd.Read()) list.Add(Read(rd));
        return list;
    }

    private static CashTransaction Read(SqliteDataReader rd) => new()
    {
        Id = rd.GetInt64(0),
        AccountId = rd.GetInt64(1),
        Date = DateOnly.ParseExact(rd.GetString(2), "yyyy-MM-dd"),
        DocumentNumber = rd.GetString(3),
        HistoryId = rd.GetInt64(4),
        AmountCents = rd.GetInt64(5),
        IsCleared = rd.GetInt64(6) != 0,
        IsArchived = rd.GetInt64(7) != 0,
        LegacyRolledIntoOpeningBalance = rd.GetInt64(8) != 0,
        Description = rd.GetString(9),
        LegacySource = rd.IsDBNull(10) ? null : rd.GetString(10),
        LegacyRecordNumber = rd.IsDBNull(11) ? null : rd.GetInt32(11),
        HistoryCode = rd.GetString(12),
        HistoryDescription = rd.GetString(13),
        Direction = rd.GetInt32(14),
        AccountCode = rd.GetString(15),
        AccountName = rd.GetString(16)
    };

    private const string BaseSelect = """
        SELECT t.id,t.account_id,t.date,t.document_number,t.history_id,t.amount_cents,
               t.is_cleared,t.is_archived,t.legacy_rolled_into_opening_balance,t.description,
               t.legacy_source,t.legacy_record_number,
               h.code,h.description,h.direction,a.code,a.name
        FROM transactions t
        JOIN histories h ON h.id=t.history_id
        JOIN accounts a ON a.id=t.account_id
        """;
}
