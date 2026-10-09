using Caixa.Core;
using Caixa.Core.Models;
using Caixa.Core.Utilities;
using Caixa.WinForms.Forms;
using Microsoft.Data.Sqlite;

internal static partial class Program
{
    private static void AuditTransactions(string root)
    {
        var failures = new List<string>();
        (string Name, Action<CaixaApplication, Account, Account, HistoryItem, HistoryItem> Test)[] cases =
        [
            ("Ciclo financeiro completo", AuditLifecycle),
            ("Períodos, filtros e arredondamento", AuditReports),
            ("Rejeição de gravação inválida", AuditValidation),
            ("Sequência combinada com referência independente", AuditSequence),
            ("Interface: tentativa inválida não altera retrato carregado", AuditEditorSnapshot),
            ("Compatibilidade: edição protegida", (a, c, _, h, _) => AuditProtected(a, c, h, "save")),
            ("Compatibilidade: exclusão protegida", (a, c, _, h, _) => AuditProtected(a, c, h, "delete")),
            ("Compatibilidade: compensação protegida", (a, c, _, h, _) => AuditProtected(a, c, h, "clear")),
            ("Compatibilidade: desarquivamento protegido", (a, c, _, h, _) => AuditProtected(a, c, h, "unarchive")),
            ("Arquivamento corresponde à consulta", AuditArchive),
            ("Atualização de registro inexistente", AuditMissing),
            ("Interface: Novo preserva edição", (a,c,d,h,s) => AuditNavigation(a,c,d,h,s,"new")),
            ("Interface: seleção preserva edição", (a,c,d,h,s) => AuditNavigation(a,c,d,h,s,"row")),
            ("Interface: conta preserva edição", (a,c,d,h,s) => AuditNavigation(a,c,d,h,s,"account"))
        ];
        foreach (var (name, test) in cases)
        {
            var app = new CaixaApplication(Path.Combine(root, "audit-" + Guid.NewGuid().ToString("N") + ".db"));
            var account = new Account { Code = "01", Name = "Principal", OpeningBalanceCents = 100000 };
            var other = new Account { Code = "02", Name = "Outra", OpeningBalanceCents = -10000 };
            // Códigos deliberadamente invertidos: o sentido vem de Direction.
            var income = new HistoryItem { Code = "99", Description = "Entrada", Direction = 1 };
            var expense = new HistoryItem { Code = "01", Description = "Saída", Direction = -1 };
            app.Accounts.Save(account); app.Accounts.Save(other);
            app.Histories.Save(income); app.Histories.Save(expense);
            try { test(app, account, other, income, expense); Console.WriteLine("PASS AUDITORIA: " + name); }
            catch (Exception ex) { failures.Add(name + ": " + ex.GetBaseException().Message); }
        }
        if (failures.Count > 0) throw new Exception("AUDITORIA - falhas demonstradas:\n" + string.Join("\n", failures));
    }

    private static CashTransaction Add(CaixaApplication app, Account account, HistoryItem history, long cents, int day, bool cleared = false)
    {
        var tx = new CashTransaction { AccountId=account.Id, HistoryId=history.Id, AmountCents=cents,
            Date=new DateOnly(2026,1,day), IsCleared=cleared, DocumentNumber="D"+day, Description="Movimento " + day };
        app.Transactions.Save(tx);
        return tx;
    }

    private static void Balances(CaixaApplication app, Account account, long real, long cleared, string stage)
    {
        var actual = app.Balances.Calculate(account.Id);
        Equal(real, actual.RealBalanceCents, stage + " / real");
        Equal(cleared, actual.ClearedBalanceCents, stage + " / bancário");
        Equal(real-cleared, actual.PendingNetCents, stage + " / pendente");
    }

    private static void AuditLifecycle(CaixaApplication app, Account a, Account b, HistoryItem income, HistoryItem expense)
    {
        Balances(app,a,100000,100000,"inicial");
        var credit=Add(app,a,income,20000,10,true);
        var debit=Add(app,a,expense,15000,11);
        Balances(app,a,105000,120000,"entrada compensada e saída pendente");
        app.Transactions.SetCleared(debit.Id,true);
        Balances(app,a,105000,105000,"compensação");
        app.Transactions.SetCleared(debit.Id,true);
        Balances(app,a,105000,105000,"compensação repetida não duplica");
        app.Transactions.SetCleared(debit.Id,false);
        Balances(app,a,105000,120000,"desfazer");
        debit.AmountCents=25000;
        app.Transactions.Save(debit);
        Balances(app,a,95000,120000,"editar valor");
        debit.HistoryId=income.Id;
        app.Transactions.Save(debit);
        Balances(app,a,145000,120000,"trocar histórico do movimento");
        debit.AccountId=b.Id;
        app.Transactions.Save(debit);
        Balances(app,a,120000,120000,"troca de conta: origem");
        Balances(app,b,15000,-10000,"troca de conta: destino");
        debit.Date=new DateOnly(2025,12,31);
        debit.IsCleared=true;
        app.Transactions.Save(debit);
        Balances(app,b,15000,15000,"editar data e compensação");
        Equal(15000L,app.Balances.CalculateBefore(b.Id,new DateOnly(2026,1,1)),"movimento anterior ao período");
        Equal(1,app.Transactions.ArchiveCleared(a.Id,new DateOnly(2026,1,10),new DateOnly(2026,1,10)),"arquivar data exata");
        Balances(app,a,120000,120000,"arquivamento");
        Equal(0,app.Transactions.ArchiveCleared(a.Id,new DateOnly(2026,1,10),new DateOnly(2026,1,10)),"arquivamento idempotente");
        app.Transactions.Unarchive(credit.Id);
        Balances(app,a,120000,120000,"desarquivamento");
        app.Transactions.Delete(credit.Id); app.Transactions.Delete(debit.Id);
        Balances(app,a,100000,100000,"exclusão origem");
        Balances(app,b,-10000,-10000,"exclusão destino");
        var reopened=new CaixaApplication(app.Database.DatabasePath);
        Balances(reopened,a,100000,100000,"persistência após reabrir");
    }

    private static void AuditReports(CaixaApplication app, Account a, Account b, HistoryItem income, HistoryItem expense)
    {
        Add(app,a,income,10000,1,true);
        var first=Add(app,a,expense,3000,10,true);
        var second=Add(app,a,income,5000,10);
        Add(app,a,expense,7000,20);
        Add(app,a,income,9000,21);
        Add(app,b,income,88888,10);
        var start=new DateOnly(2026,1,10); var end=new DateOnly(2026,1,20);
        app.Transactions.ArchiveCleared(a.Id,start,start);
        var rows=app.Reports.Statement(a.Id,start,end);
        Equal("107000,112000,105000",string.Join(',',rows.Select(x=>x.RunningBalanceCents)),"extrato esperado linha a linha");
        Equal(first.Id,rows[0].TransactionId,"ordem mesma data: primeiro ID");
        Equal(second.Id,rows[1].TransactionId,"ordem mesma data: segundo ID");
        Equal(110000L,app.Balances.CalculateBefore(a.Id,start),"saldo anterior");
        Equal(105000L,app.Balances.CalculateBefore(a.Id,end.AddDays(1)),"saldo até fim inclusivo");
        Equal(2,app.Reports.Consistency(a.Id,start,end).Count,"pendências no período");
        Equal(1,app.Reports.Archived(a.Id,start,end).Count,"arquivados no período");
        Equal(-10000L,app.Reports.ByHistories(a.Id,start,end,[expense.Id]).Sum(x=>x.AmountCents*x.Direction),"total por histórico");
        var criteria=new TransactionSearchCriteria {AccountId=a.Id,StartDate=start,EndDate=end,Archived=null};
        Equal(3,app.Reports.SearchTransactions(criteria).Count,"consulta concorda com extrato");
        criteria.Cleared=false; criteria.MinimumAmountCents=5000; criteria.MaximumAmountCents=5000;
        Equal(second.Id,app.Reports.SearchTransactions(criteria).Single().Id,"combinação de filtros e valor inclusivo");
        Equal(101L,Money.ToCents(1.005m),"arredondamento positivo");
        Equal(-101L,Money.ToCents(-1.005m),"arredondamento negativo");
        Equal(123456.78m,Money.FromCents(Money.ToCents(123456.78m)),"centavos exatos");
    }

    private static void AuditValidation(CaixaApplication app, Account a, Account b, HistoryItem income, HistoryItem expense)
    {
        var tx=Add(app,a,income,1000,1);
        tx.AmountCents=0;
        Reject<InvalidOperationException>(()=>app.Transactions.Save(tx),"zero");
        tx.AmountCents=-1;
        Reject<InvalidOperationException>(()=>app.Transactions.Save(tx),"negativo");
        tx.AmountCents=2000; tx.HistoryId=9999;
        Reject<SqliteException>(()=>app.Transactions.Save(tx),"histórico inexistente");
        tx.HistoryId=income.Id; tx.AccountId=9999;
        Reject<SqliteException>(()=>app.Transactions.Save(tx),"conta inexistente");
        Balances(app,a,101000,100000,"falhas não alteram valor persistido");
        Equal(1,app.Transactions.GetByAccount(a.Id).Count,"falhas não duplicam registro");
        Reject<InvalidOperationException>(()=>app.Accounts.Delete(a.Id),"exclusão de conta usada");
        Reject<InvalidOperationException>(()=>app.Histories.Delete(income.Id),"exclusão de histórico usado");
    }

    private static void AuditProtected(CaixaApplication app, Account a, HistoryItem income, string action)
    {
        var tx=Add(app,a,income,1000,1,true);
        Sql(app.Database,"UPDATE transactions SET legacy_rolled_into_opening_balance=1,is_archived=1;");
        tx.LegacyRolledIntoOpeningBalance=false; // A proteção deve consultar o banco, não confiar no objeto.
        Action operation=action switch {
            "save" => ()=>app.Transactions.Save(tx), "delete" => ()=>app.Transactions.Delete(tx.Id),
            "clear" => ()=>app.Transactions.SetCleared(tx.Id,false), _ => ()=>app.Transactions.Unarchive(tx.Id) };
        Reject<InvalidOperationException>(operation,"registro incorporado protegido: "+action);
        Equal(1,app.Transactions.GetByAccount(a.Id,true).Count,"registro preservado");
        Balances(app,a,100000,100000,"incorporado não contado novamente");
    }

    private static void AuditArchive(CaixaApplication app, Account a, Account b, HistoryItem income, HistoryItem expense)
    {
        var old=Add(app,a,income,1000,1,true);
        Sql(app.Database,"UPDATE transactions SET legacy_rolled_into_opening_balance=1;");
        Add(app,a,expense,2000,1,true); Add(app,a,expense,3000,1);
        Equal(1,app.Transactions.ArchiveCleared(a.Id,new DateOnly(2026,1,1),new DateOnly(2026,1,1)),"somente compensado exibido na consulta é arquivado");
        Equal(false,app.Transactions.GetByAccount(a.Id,true).Single(t=>t.Id==old.Id).IsArchived,"registro de compatibilidade intocado");
        Balances(app,a,95000,98000,"arquivar preserva saldos e pendências");
    }

    private static void AuditMissing(CaixaApplication app, Account a, Account b, HistoryItem income, HistoryItem expense)
    {
        var tx=Add(app,a,income,1000,1);
        app.Transactions.Delete(tx.Id);
        Reject<InvalidOperationException>(()=>app.Transactions.Save(tx),"UPDATE inexistente não pode indicar sucesso");
        Reject<InvalidOperationException>(()=>app.Transactions.SetCleared(tx.Id,true),"compensação inexistente");
        Reject<InvalidOperationException>(()=>app.Transactions.Unarchive(tx.Id),"desarquivamento inexistente");
        Reject<InvalidOperationException>(()=>app.Transactions.Delete(tx.Id),"exclusão inexistente");
    }

    private sealed record ExpectedMovement(long Id, long Account, decimal SignedAmount, int Day, bool Cleared, bool Archived);

    private static void AuditSequence(CaixaApplication app, Account a, Account b, HistoryItem income, HistoryItem expense)
    {
        var random=new Random(20261002);
        var expected=new Dictionary<long,ExpectedMovement>();
        var start=new DateOnly(2026,1,1); var end=new DateOnly(2026,1,31);
        for(var step=0;step<120;step++)
        {
            var operation=expected.Count==0 ? 0 : random.Next(6);
            var account=random.Next(2)==0 ? a : b;
            var history=random.Next(2)==0 ? income : expense;
            var cents=random.Next(1,50001);
            var day=random.Next(1,32);
            var cleared=random.Next(2)==0;
            var current=expected.Count==0 ? null : expected.Values.ElementAt(random.Next(expected.Count));
            switch(operation)
            {
                case 0:
                    var added=Add(app,account,history,cents,day,cleared);
                    expected.Add(added.Id,new(added.Id,account.Id,(history==income?cents:-cents)/100m,day,cleared,false));
                    break;
                case 1:
                    app.Transactions.Save(new CashTransaction {Id=current!.Id, AccountId=account.Id,
                        HistoryId=history.Id,AmountCents=cents,Date=new DateOnly(2026,1,day),
                        IsCleared=cleared,IsArchived=current.Archived});
                    expected[current.Id]=current with {Account=account.Id,SignedAmount=(history==income?cents:-cents)/100m,Day=day,Cleared=cleared};
                    break;
                case 2:
                    app.Transactions.SetCleared(current!.Id,cleared);
                    expected[current.Id]=current with {Cleared=cleared};
                    break;
                case 3:
                    var targets=expected.Values.Where(x=>x.Account==account.Id && x.Cleared && !x.Archived).ToList();
                    Equal(targets.Count,app.Transactions.ArchiveCleared(account.Id,start,end),"quantidade de baixas na sequência");
                    foreach(var target in targets) expected[target.Id]=target with {Archived=true};
                    break;
                case 4:
                    app.Transactions.Unarchive(current!.Id);
                    expected[current.Id]=current with {Archived=false};
                    break;
                case 5:
                    app.Transactions.Delete(current!.Id); expected.Remove(current.Id);
                    break;
            }
            foreach(var owner in new[]{a,b})
            {
                var ledger=expected.Values.Where(x=>x.Account==owner.Id).OrderBy(x=>x.Day).ThenBy(x=>x.Id).ToList();
                var opening=owner.OpeningBalanceCents/100m;
                var balance=app.Balances.Calculate(owner.Id);
                Equal(opening+ledger.Sum(x=>x.SignedAmount),balance.RealBalanceCents/100m,$"sequência {step}: real");
                Equal(opening+ledger.Where(x=>x.Cleared).Sum(x=>x.SignedAmount),balance.ClearedBalanceCents/100m,$"sequência {step}: bancário");
                Equal(ledger.Count,app.Transactions.GetByAccount(owner.Id,true).Count,"quantidade persistida");
                var statement=app.Reports.Statement(owner.Id,start,end);
                Equal(ledger.Count,statement.Count,"quantidade no extrato");
                var running=opening;
                for(var i=0;i<ledger.Count;i++)
                {
                    running+=ledger[i].SignedAmount;
                    Equal(ledger[i].Id,statement[i].TransactionId,"ordem dos movimentos");
                    Equal(running,statement[i].RunningBalanceCents/100m,"saldo acumulado independente");
                }
            }
        }
    }

    private static void AuditEditorSnapshot(CaixaApplication app, Account a, Account b, HistoryItem income, HistoryItem expense)
    {
        Add(app,a,income,1000,1);
        using var form=new TransactionsForm(app,()=>false);
        form.BindingContext=new BindingContext(); Call(form,"LoadCombos");
        var grid=Field<DataGridView>(form,"grid"); grid.CurrentCell=grid.Rows[0].Cells[0]; Call(form,"SelectCurrent");
        Field<NumericUpDown>(form,"numAmount").Value=0;
        Field<CheckBox>(form,"chkCleared").Checked=true;
        var draft=(CashTransaction)Call(form,"CaptureTransaction")!;
        Reject<InvalidOperationException>(()=>app.Transactions.Save(draft),"rascunho inválido");
        Equal(false,Field<CashTransaction>(form,"_loaded").IsCleared,"falha não altera estado carregado");
        Equal(1000L,Field<CashTransaction>(form,"_loaded").AmountCents,"falha não altera valor carregado");
        Field<NumericUpDown>(form,"numAmount").Value=10;
        Field<CheckBox>(form,"chkCleared").Checked=false;
        Call(form,"ToggleCleared");
        Equal(true,app.Transactions.GetByAccount(a.Id).Single().IsCleared,"compensação após correção do rascunho");
    }

    private static void AuditNavigation(CaixaApplication app, Account a, Account b, HistoryItem income, HistoryItem expense, string operation)
    {
        Add(app,a,income,1000,1); Add(app,a,expense,500,2);
        var confirmCount=0;
        var discard=false;
        using var form=new TransactionsForm(app, () => { confirmCount++; return discard; });
        form.BindingContext=new BindingContext();
        Call(form,"LoadCombos");
        var grid=Field<DataGridView>(form,"grid");
        grid.CurrentCell=grid.Rows[0].Cells[0]; Call(form,"SelectCurrent");
        Field<TextBox>(form,"txtDescription").Text="EDIÇÃO A PRESERVAR";
        if(operation=="new") Call(form,"NewItem");
        else if(operation=="row") Equal(false,grid.Enabled,"navegação na grade bloqueada enquanto há edições");
        else Equal(false,Field<ComboBox>(form,"cmbAccount").Enabled,"troca de conta bloqueada enquanto há edições");
        Equal("EDIÇÃO A PRESERVAR",Field<TextBox>(form,"txtDescription").Text,"edição não descartada silenciosamente");
        Equal(false,Field<CheckBox>(form,"chkShowArchived").Enabled,"filtro não pode ocultar o registro editado");
        if(operation=="new")
        {
            Equal(1,confirmCount,"Novo solicita confirmação");
            discard=true;
            Call(form,"NewItem");
            Equal(2,confirmCount,"descarte confirmado");
            Equal("",Field<TextBox>(form,"txtDescription").Text,"descarte inicia novo registro");
        }
        else
        {
            Equal(true,(bool)Call(form,"SaveItem")!,"salvar libera navegação");
            Equal(true,grid.Enabled,"grade liberada após salvar");
            Equal(true,Field<ComboBox>(form,"cmbAccount").Enabled,"conta liberada após salvar");
            Field<ComboBox>(form,"cmbAccount").SelectedIndex=1;
            Equal(0,confirmCount,"troca de conta sem alterações não pede descarte");
            Equal("",Field<TextBox>(form,"txtDescription").Text,"troca de conta limpa editor");
        }
    }
}
