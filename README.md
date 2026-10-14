# Contas — estudo WebAssembly e SaaS

A branch `codex/saas-cycle-5` contém o quinto ciclo do SaaS em **Rust + WebAssembly + PostgreSQL**: relatórios por conta/histórico, exportação CSV e paginação SQL, além de lançamentos e isolamento dos dados. Veja [execução e testes](saas/README.md).

O código C#/SQLite permanece em `src`, com seus scripts, testes e documentação. Nenhum dado desktop foi convertido. O projeto desktop e suas releases continuam no [repositório original](https://github.com/pastrello/contas).

Este repositório independente preserva o histórico do original e a base desktop auditada 1.6.2. A branch `main` mantém essa base; `codex/saas-rust-cycle-1` mantém o primeiro ciclo. O desenvolvimento atual está em `codex/saas-cycle-5`.

- [Projeto SaaS e instruções](saas/README.md)
- [Referência desktop](docs/README-DESKTOP.md)
- [Auditoria financeira herdada](docs/10-AUDITORIA-LANCAMENTOS.md)

Os scripts de instalador da raiz pertencem ao desktop, não ao SaaS. A versão 1.6.2 na raiz identifica a base desktop; o workspace Rust está em 0.5.0. A versão web é um protótipo de estudo, com lançamentos e saldos financeiros.
