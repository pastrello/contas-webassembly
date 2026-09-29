using Caixa.Core.Models;
using Caixa.Core.Repositories;

namespace Caixa.Core.Services;

/// <summary>
/// Regra central de cálculo financeiro.
/// Entrada soma, saída subtrai. Saldo bancário considera somente lançamentos contabilizados.
/// </summary>
public sealed class BalanceService
{
    private readonly AccountRepository _accounts;
    private readonly TransactionRepository _transactions;

    public BalanceService(AccountRepository accounts, TransactionRepository transactions)
    {
        _accounts = accounts;
        _transactions = transactions;
    }

    public BalanceSnapshot Calculate(long accountId)
    {
        var account = _accounts.GetById(accountId) ?? throw new InvalidOperationException("Conta não encontrada.");
        var real = account.OpeningBalanceCents;
        var cleared = account.OpeningBalanceCents;
        foreach (var tx in _transactions.GetAllIncluded(accountId))
        {
            var signed = tx.AmountCents * tx.Direction;
            real += signed;
            if (tx.IsCleared) cleared += signed;
        }
        return new BalanceSnapshot(account.OpeningBalanceCents, real, cleared, real - cleared);
    }

    /// <summary>Saldo real imediatamente anterior ao período, inclusive movimentos arquivados.</summary>
    public long CalculateBefore(long accountId, DateOnly start)
    {
        var account = _accounts.GetById(accountId) ?? throw new InvalidOperationException("Conta não encontrada.");
        var running = account.OpeningBalanceCents;
        foreach (var tx in _transactions.GetIncludedBefore(accountId, start))
            running += tx.AmountCents * tx.Direction;
        return running;
    }

    public List<StatementLine> BuildStatement(long accountId, DateOnly start, DateOnly end)
    {
        var running = CalculateBefore(accountId, start);

        var rows = _transactions.GetByPeriod(accountId, start, end)
            .Where(t => !t.LegacyRolledIntoOpeningBalance)
            .OrderBy(t => t.Date).ThenBy(t => t.Id)
            .ToList();

        var result = new List<StatementLine>();
        foreach (var tx in rows)
        {
            running += tx.AmountCents * tx.Direction;
            result.Add(new StatementLine
            {
                TransactionId = tx.Id,
                Date = tx.Date,
                DocumentNumber = tx.DocumentNumber,
                HistoryCode = tx.HistoryCode,
                HistoryDescription = tx.HistoryDescription,
                Description = tx.Description,
                AmountCents = tx.AmountCents,
                Direction = tx.Direction,
                IsCleared = tx.IsCleared,
                IsArchived = tx.IsArchived,
                RunningBalanceCents = running
            });
        }
        return result;
    }
}
