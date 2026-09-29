# Correções de integridade e relatórios

## Comportamento

- Um histórico com lançamentos (inclusive arquivados) não pode trocar Entrada/Saída. A descrição e o estado ativo continuam editáveis. Para outra direção, cadastre outro histórico.
- Em lançamentos, salve as edições antes de usar **Contabilizar / desfazer**. O botão permanece desabilitado enquanto houver alterações pendentes. O checkbox Contabilizado continua fazendo parte da edição normal gravada por Salvar.
- Um lançamento existente pode manter seu histórico desativado ao editar outros campos. Novos lançamentos oferecem somente históricos ativos.
- Os PDFs usam conta, período, filtros e totais da consulta exibida. Alterar filtros sem consultar novamente não muda o cabeçalho dos dados já carregados.
- Um extrato sem movimentos mostra o saldo anterior como saldo final.
- A restauração rejeita colunas ausentes e referências inválidas antes de substituir o banco. Continua criando backup preventivo e tentando rollback em caso de falha posterior.
- A abertura de um banco com `user_version` superior à versão suportada é rejeitada sem sobrescrever a versão.

Não há migração de esquema nem remoção de campos de compatibilidade. TAB e SHIFT+TAB permanecem como estavam.

## Classes alteradas

- `HistoryRepository`: valida a direção dentro da mesma transação da gravação, evitando que a verificação e a alteração sejam separadas por outra escrita.
- `TransactionsForm`: protege edições pendentes na contabilização e inclui o histórico inativo original apenas ao editar um movimento existente.
- `BalanceService`: centraliza o saldo anterior ao período em `CalculateBefore`, reutilizado pelo extrato.
- `PeriodReportForm`, `HistoryReportForm`, `TransactionSearchForm`: guardam os metadados da consulta junto aos resultados usados pelo PDF.
- `BackupService`: verifica colunas e vínculos; aceita opcionalmente uma pasta de backup preventivo para permitir testes isolados. O caminho padrão da aplicação permanece o mesmo.
- `Database`: protege versões futuras e descarta a conexão se sua abertura/configuração falhar.
- `MainForm`: descarta também os diálogos de consultas, relatórios, manutenção e Sobre.

## Testes

O executável `tests/Caixa.RegressionTests` não exige novos pacotes de testes. Ele usa bancos e ZIPs descartáveis em uma subpasta com identificador único. Não abre janelas e não acessa o banco operacional.

No terminal normal, na raiz do projeto:

```bat
dotnet run --project tests\Caixa.RegressionTests -c Release -- .build-codex\regression-data
```

No ambiente Codex com as dependências locais já preparadas:

```bat
test-codex.cmd
```

O segundo caminho usa os arquivos locais de apoio `build-codex.cmd` e `NuGet.Codex.Config`; não executa auditoria online de vulnerabilidades. O executável retorna código 1 quando uma verificação falha.

Cobertura: direção de históricos usados/livres, movimentos arquivados, saldo anterior, compatibilidade de movimentos incorporados, backup/restauração com WAL, rejeição de snapshots inválidos, versão futura, edição com histórico inativo, estado do botão de contabilização e retenção dos filtros nas três telas de relatório.

As verificações de Forms exercitam controles e métodos sem mostrar janelas. Não substituem a inspeção visual do PDF ou um teste manual de navegação. Os dados gerados são mantidos na pasta ignorada `.build-codex` para diagnóstico.

## Melhorias separadas desta etapa

A confirmação ao clicar em Novo ou trocar de registro continua como melhoria futura: a proteção de descarte existente atua no fechamento. Também permanecem a pequena limpeza de métodos sem uso, a regra de cheques pelo código 51 e os três avisos anteriores de nulabilidade.
