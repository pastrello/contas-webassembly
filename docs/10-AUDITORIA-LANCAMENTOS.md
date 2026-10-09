# Auditoria dos lançamentos — 02/10/2026

## Resultado

Compilação Release aprovada, com os três avisos anteriores de nulabilidade.
A suíte completa passou em **2.464 verificações**, incluindo as 36 anteriores.
As verificações novas estão organizadas em 14 grupos; um deles executa uma
sequência determinística de 120 operações combinadas, comparando os resultados
com um registro independente em memória. O total conta comparações de valores,
não 2.464 cenários independentes.

Nenhum banco operacional foi acessado. Foram criados bancos SQLite descartáveis
sob `.build-codex/regression-data`. Os testes executam os repositórios, serviços e
controles reais do projeto. A etapa não gerou novo instalador nem publicou release.

## Regras confirmadas nos cenários testados

- Saldo real = inicial + entradas - saídas; saldo bancário considera somente compensados.
- A direção vem do histórico explícito, inclusive com entrada de código 99 e saída de código 01.
- Inclusão, edição do valor, troca de histórico e troca de conta recalculam corretamente os saldos.
- Trocar a conta de um movimento pelo repositório retira seu efeito da origem e o inclui no destino.
- Compensar/desfazer altera o saldo bancário sem alterar o real; repetir a compensação não duplica valores.
- Arquivar/desarquivar preserva os saldos; excluir retira o efeito do movimento.
- Movimentos anteriores ao período entram no saldo anterior; as datas inicial/final são inclusivas no extrato.
- Movimentos da mesma data têm ordem estável por ID. Extrato, consulta e totais por histórico concordam nos filtros testados.
- Valores em centavos e arredondamento decimal produzem os resultados esperados.
- Gravações de valor zero/negativo e referências inexistentes são rejeitadas sem modificar o registro persistido.
- Contas e históricos utilizados não podem ser excluídos.
- Reabrir o banco preserva os resultados gravados.
- Registros já incorporados ao saldo inicial não são contados novamente.

Exemplo explícito: inicial R$ 1.000, entrada compensada R$ 200 e saída pendente
R$ 150 resultaram em real R$ 1.050 e bancário R$ 1.200. Ao compensar a saída,
ambos ficaram em R$ 1.050. O teste prosseguiu com alterações e exclusões.

## Problemas demonstrados e corrigidos

| Achado | Evidência antes da correção | Correção |
| --- | --- | --- |
| Descarte silencioso de edição | Novo, seleção de outro registro e troca de conta apagavam o complemento ainda não salvo | A navegação da tela de lançamentos fica desabilitada enquanto há edição. Salvar libera os controles; Novo pede confirmação antes de descartar. O título explica o estado. |
| Proteção de compatibilidade apenas na interface | As quatro operações diretas do repositório alteravam/excluíam/compensavam/desarquivavam um registro incorporado | O SQL exige `legacy_rolled_into_opening_balance=0`, mesmo que o objeto recebido afirme não estar protegido. |
| Baixa diferente da consulta | O repositório arquivava dois movimentos quando só um era elegível na consulta; o outro era de compatibilidade | O arquivamento em lote usa o mesmo filtro de compatibilidade da tela. |
| Sucesso falso ao salvar registro removido | Após excluir um movimento, Save com o ID antigo retornava sucesso | UPDATE usa `RETURNING id`; ausência de linha retorna erro. Exclusão, compensação e desarquivamento também verificam a quantidade afetada. |

A execução inicial registrou nove falhas: quatro operações de compatibilidade,
três caminhos de navegação, o arquivamento e a atualização de ID inexistente.
O ciclo financeiro básico e os relatórios já passavam antes dessas correções.

Na revisão do código também foi corrigido o uso do mesmo objeto carregado como
rascunho de edição. Agora `CaptureTransaction` cria outro objeto e a tela só
substitui o estado carregado após a gravação. O teste rejeita um rascunho inválido,
confirma a preservação do estado anterior e depois verifica a compensação.
Também foi adicionada validação de período invertido ao arquivamento.

## Arquivos alterados

- `TransactionRepository`: proteção no SQL e confirmação de operações realmente aplicadas.
- `TransactionsForm`: estado de edição, descarte confirmado e separação do rascunho.
- `UnsavedChangesGuard`: confirmação de descarte para Novo.
- `Properties/AssemblyInfo.cs`: acesso interno restrito ao projeto de testes.
- `tests/Caixa.RegressionTests/TransactionAudit.cs`: cenários e referência independente.
- `tests/Caixa.RegressionTests/Program.cs`: execução da nova auditoria após a suíte anterior.

O esquema SQLite, as fórmulas financeiras e a navegação por TAB/SHIFT+TAB não mudaram.
Não foi criada rotina de migração.

## Repetir e interpretar os testes

Na raiz do projeto, execute `test-codex.cmd` no ambiente já preparado. No terminal
normal, também é possível executar:

```bat
dotnet run --project tests\Caixa.RegressionTests -c Release -- .build-codex\regression-data
```

Os testes de interface instanciam Forms e controles sem mostrar janelas.
A confirmação de descarte é substituída por uma resposta controlada somente no
construtor interno usado pelos testes; o construtor público continua exibindo
a confirmação normal. São verificados cancelamento, aceitação, controles
desabilitados durante a edição e liberação após Salvar.

## Limites

- Não houve teste visual de foco, teclado físico, caixas de diálogo ou layout de PDF.
- A concordância dos relatórios foi verificada nos dados e metadados, não em novos PDFs renderizados.
- A sequência combinada cobre operações reproduzíveis, não todas as combinações possíveis.
- Não houve teste multiusuário nem de falha de energia, fora do escopo atual.
- A proteção de navegação adicionada nesta etapa se aplica à tela de lançamentos; os cadastros de contas/históricos ainda merecem a mesma melhoria em outra etapa.
- A previsão de cheques continua usando o código 51, conforme a regra existente.
- As correções estão nos fontes locais. O instalador v1.6.1 anteriormente publicado não foi substituído por esta auditoria.
