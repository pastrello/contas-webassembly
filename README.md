# Contas — estudo WebAssembly e SaaS

A branch `codex/saas-rust-cycle-1` inicia o SaaS em **Rust + WebAssembly + PostgreSQL**, na pasta [`saas`](saas/README.md).

O código C#/SQLite permanece preservado em `src`, com seus scripts, testes e documentação. Nenhum dado foi convertido. O projeto desktop e suas releases continuam no [repositório original](https://github.com/pastrello/contas).

Este repositório independente preserva o histórico do original e a base desktop auditada 1.6.2. A branch `main` mantém essa base; o desenvolvimento SaaS ocorre nesta branch. O plano inicial Blazor/SQLite foi substituído por Rust/PostgreSQL conforme a direção escolhida para estudar WebAssembly e SaaS.

- [Executar e testar o ciclo 1](saas/README.md)
- [Referência desktop](docs/README-DESKTOP.md)
- [Auditoria financeira herdada](docs/10-AUDITORIA-LANCAMENTOS.md)

Os scripts de instalador da raiz pertencem ao desktop, não ao SaaS. Login e isolamento de clientes estão previstos para o ciclo 2; este primeiro ciclo demonstra somente navegador → API → PostgreSQL.
