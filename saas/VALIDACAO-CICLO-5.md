# Validação do ciclo 5 — 14/10/2026

Branch codex/saas-cycle-5; workspace Rust 0.5.0.

## Aprovado

- Compilação API Windows e frontend WebAssembly.
- Quatro testes unitários: indisponibilidade/autenticação, dinheiro exato, calendário e escaping/proteção CSV.
- Clippy API/all-targets e frontend WASM sem avisos.
- Regressão integrada dos ciclos 2 a 5 em PostgreSQL descartável, incluindo gravações simultâneas com quatro conexões.
- Resumo por conta: saldos inicial, anterior, entradas, saídas, final e bancário até o fim do período; total consolidado exato.
- Totais por histórico: movimentação por período, sem saldos iniciais.
- Períodos inclusivos, datas inválidas/invertidas rejeitadas, cadastros inativos incluídos e cancelados excluídos.
- Mesmo relatório para administrador e consulta do mesmo ambiente; dados de outro ambiente ausentes.
- CSV autenticado, cabeçalhos de download, no-store, BOM, aspas/quebras de linha e neutralização de fórmula em nome de conta.
- A regressão financeira e a paginação de 201 lançamentos passaram com o novo SQL: 200/1 linhas por página, ordem estável e totais íntegros.
- Dez verificações HTTP reais de disponibilidade, arquivos WASM e comportamento com banco desligado.

## Limites

Sem verificação visual automatizada por bloqueio da ferramenta de navegador. Linux/Docker e carga não executados. As agregações SQL ainda percorrem os registros pertinentes; relatórios retornam todos os cadastros agregados. O CSV é recalculado no download e pode refletir gravações posteriores à tela.

Não houve migração de esquema neste ciclo. O ambiente local utiliza o procedimento existente de backup completo e comparação dos dados antes/depois. Desktop e releases permanecem intactos.
